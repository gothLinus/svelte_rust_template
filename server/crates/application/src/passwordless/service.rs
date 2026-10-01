use std::sync::Arc;

use domain::{
    audit::AuthMethod,
    clock::Clock,
    database::{Database, Transaction},
    i18n::Message,
    one_time_code::{CODE_TTL, CodeChannel, CodePurpose, OneTimeCodeRepository},
    secret::Secret,
    security::TokenGenerator,
    session::ClientInfo,
    text::TextMessage,
    user::{Email, PhoneNumber, User, UserRepository},
    user_token::{TokenPurpose, UserTokenRepository},
};

use crate::{
    Adapters, Context,
    auth::{
        access,
        signin::{self, LoginOutcome},
    },
    codes,
    error::{AppError, ErrorChain},
    mail,
    passwordless::dto::{
        EmailCodeRequest, MagicLinkRequest, PhoneCodeRequest, VerifyEmailCodeRequest,
        VerifyPhoneCodeRequest,
    },
    tokens,
};

const TEXT_CHANNELS: [CodeChannel; 2] = [CodeChannel::Sms, CodeChannel::Whatsapp];

/// Sign-in without a password. Every method is a first step: it ends in
/// `signin::complete_first_step`, so a user with a second step still has to take it.
pub struct PasswordlessService<A: Adapters> {
    ctx: Arc<Context<A>>,
}

impl<A: Adapters> PasswordlessService<A> {
    pub fn new(ctx: Arc<Context<A>>) -> Self {
        Self { ctx }
    }

    /// Mails a magic link and a code to the address, if an enabled account uses it. Succeeds either
    /// way, so the response does not reveal which addresses have one. A new link and code replace
    /// the previous ones.
    pub async fn request_email_code(&self, request: EmailCodeRequest) -> Result<(), AppError> {
        let email = Email::parse(&request.email).map_err(|err| AppError::invalid("email", &err))?;
        let mut tx = self.ctx.db.transaction().await?;
        let Some(user) = tx
            .find_user_by_email(&email)
            .await?
            .filter(|user| !user.is_disabled())
        else {
            return Ok(());
        };

        let now = self.ctx.clock.now();
        let token =
            tokens::issue(&self.ctx, &mut tx, user.id(), TokenPurpose::MagicLink, now).await?;
        let code = codes::issue(
            &self.ctx,
            &mut tx,
            user.id(),
            CodePurpose::Login,
            CodeChannel::Email,
            None,
        )
        .await?;
        tx.commit().await?;

        let link = self.ctx.settings.links.magic_link(&token);
        let ttl = self.ctx.settings.tokens.magic_link_ttl;
        mail::send(
            &*self.ctx.mailer,
            mail::sign_in_code(
                &self.ctx.voice_for(&user),
                user.email().clone(),
                &link,
                &group(code.expose()),
                ttl,
            ),
        )
        .await;
        tracing::info!(user_id = %user.id(), "sign-in code mailed");
        Ok(())
    }

    /// Signs in with the emailed code. It proves the user reads the mailbox, so the address counts
    /// as verified (the first such proof evicts earlier claimants, see the [`auth`](crate::auth)
    /// module).
    ///
    /// The code and the magic link from the same mail are alternatives: using one retires the
    /// other.
    ///
    /// # Errors
    ///
    /// [`AppError::invalid_code`] for an unknown address, a wrong, expired or used code, and too
    /// many wrong guesses, all the same. Wrong guesses are counted even though the request fails.
    pub async fn verify_email_code(
        &self,
        request: VerifyEmailCodeRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<LoginOutcome, AppError> {
        let email = Email::parse(&request.email).map_err(|_| AppError::invalid_code())?;
        let mut tx = self.ctx.db.transaction().await?;
        let user = tx.find_user_by_email(&email).await?;
        // An unknown address runs the same statements against an id no row has, so the answer takes
        // as long as for a known one: timing does not tell which addresses have an account.
        let checked = codes::check(
            &self.ctx,
            &mut tx,
            user.as_ref().map_or_else(nobody, User::id),
            CodePurpose::Login,
            &[CodeChannel::Email],
            request.code.0.expose(),
        )
        .await;
        tx.commit().await?;
        checked?;
        let user = user.ok_or_else(AppError::invalid_code)?;

        let user = self.email_proven(&user, &client).await?;
        signin::complete_first_step(&self.ctx, &user, client, previous, AuthMethod::EmailCode).await
    }

    /// Signs in with the link from the email. The link works once, and retires the code from the
    /// same mail.
    ///
    /// # Errors
    ///
    /// [`AppError::InvalidToken`] for a link that is unknown, expired or already used.
    pub async fn magic_link(
        &self,
        request: MagicLinkRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<LoginOutcome, AppError> {
        let token = request.token.0;
        if token.is_empty() || !token.is_within_limit() {
            return Err(AppError::InvalidToken);
        }
        let user_id = self
            .ctx
            .db
            .connection()
            .await?
            .consume_user_token(
                &self.ctx.tokens.digest(&token),
                TokenPurpose::MagicLink,
                self.ctx.clock.now(),
            )
            .await?
            .ok_or(AppError::InvalidToken)?
            .user_id;
        let user = self
            .ctx
            .db
            .connection()
            .await?
            .find_user(user_id)
            .await?
            .ok_or(AppError::InvalidToken)?;

        let user = self.email_proven(&user, &client).await?;
        signin::complete_first_step(&self.ctx, &user, client, previous, AuthMethod::MagicLink).await
    }

    /// The checks of [`PasswordlessService::request_phone_code`] that do not depend on whether the
    /// number has an account: the channel is set up, the number is valid and may be texted. A
    /// caller that hands the request off runs these first.
    pub fn check_phone_request(
        &self,
        request: &PhoneCodeRequest,
    ) -> Result<(CodeChannel, PhoneNumber), AppError> {
        let channel = CodeChannel::from(request.channel);
        self.ensure_channel(channel)?;
        let phone =
            PhoneNumber::parse(&request.phone).map_err(|err| AppError::invalid("phone", &err))?;
        self.ctx.settings.ensure_textable(&phone)?;
        Ok((channel, phone))
    }

    /// Texts a code to the number, if an enabled account has verified it. Succeeds either way, like
    /// [`PasswordlessService::request_email_code`].
    pub async fn request_phone_code(&self, request: PhoneCodeRequest) -> Result<(), AppError> {
        let (channel, phone) = self.check_phone_request(&request)?;

        let mut conn = self.ctx.db.connection().await?;
        let Some(user) = conn
            .find_user_by_phone(&phone)
            .await?
            .filter(|user| user.verified_phone().is_some() && !user.is_disabled())
        else {
            return Ok(());
        };
        let code = codes::issue(
            &self.ctx,
            &mut conn,
            user.id(),
            CodePurpose::Login,
            channel,
            None,
        )
        .await?;
        drop(conn);

        let message = Message::new("sms-sign-in-code")
            .arg("code", group(code.expose()))
            .arg("app", self.ctx.settings.app_name.as_str())
            .arg("minutes", CODE_TTL.whole_minutes());
        send_text(&self.ctx, &user, phone, channel, &message).await;
        tracing::info!(user_id = %user.id(), %channel, "sign-in code texted");
        Ok(())
    }

    /// Signs in with the texted code. Unlike the emailed code this proves nothing about the
    /// address, so it does not mark it verified.
    ///
    /// # Errors
    ///
    /// [`AppError::invalid_code`] for an unknown number, a wrong, expired or used code, and too
    /// many wrong guesses, all the same.
    pub async fn verify_phone_code(
        &self,
        request: VerifyPhoneCodeRequest,
        client: ClientInfo,
        previous: Option<&Secret>,
    ) -> Result<LoginOutcome, AppError> {
        let phone = PhoneNumber::parse(&request.phone).map_err(|_| AppError::invalid_code())?;
        let mut tx = self.ctx.db.transaction().await?;
        let user = tx
            .find_user_by_phone(&phone)
            .await?
            .filter(|user| user.verified_phone().is_some());
        // Like an unknown address above: the same statements either way.
        let checked = codes::check(
            &self.ctx,
            &mut tx,
            user.as_ref().map_or_else(nobody, User::id),
            CodePurpose::Login,
            &TEXT_CHANNELS,
            request.code.0.expose(),
        )
        .await;
        tx.commit().await?;
        checked?;
        let user = user.ok_or_else(AppError::invalid_code)?;

        signin::complete_first_step(&self.ctx, &user, client, previous, AuthMethod::PhoneCode).await
    }

    fn ensure_channel(&self, channel: CodeChannel) -> Result<(), AppError> {
        if self.ctx.texts.channels().contains(&channel) {
            Ok(())
        } else {
            Err(AppError::conflict(
                "channel_unavailable",
                Message::new("conflict-channel-unavailable").arg("channel", channel.as_str()),
            ))
        }
    }

    async fn email_proven(&self, user: &User, client: &ClientInfo) -> Result<User, AppError> {
        let mut tx = self.ctx.db.transaction().await?;
        tx.delete_user_tokens(user.id(), TokenPurpose::MagicLink)
            .await?;
        tx.delete_one_time_code(user.id(), CodePurpose::Login)
            .await?;
        let proven = access::prove_email_ownership(
            &mut tx,
            user.id(),
            self.ctx.clock.now(),
            access::Prover::Owner,
            client,
        )
        .await?;
        tx.commit().await?;
        if proven.password_removed {
            access::offer_password(&self.ctx, &proven.user).await;
        }
        Ok(proven.user)
    }
}

fn nobody() -> domain::user::UserId {
    domain::user::UserId::from_uuid(uuid::Uuid::nil())
}

pub(crate) fn group(code: &str) -> String {
    if code.len() == 6 {
        format!("{} {}", &code[..3], &code[3..])
    } else {
        code.to_owned()
    }
}

/// Texts `text` to `to`, in `user`'s language.
pub(crate) async fn send_text<A: Adapters>(
    ctx: &Context<A>,
    user: &User,
    to: PhoneNumber,
    channel: CodeChannel,
    text: &Message,
) {
    let masked = to.masked();
    let message = TextMessage {
        to,
        channel,
        body: ctx.voice_for(user).say(text),
        valid_for: CODE_TTL,
    };
    if let Err(err) = ctx.texts.send(message).await {
        tracing::error!(to = masked, %channel, error = %ErrorChain(&err), "failed to send a text message");
    }
}
