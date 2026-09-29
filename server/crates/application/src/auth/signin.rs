use domain::{
    clock::Clock,
    database::{Database, Transaction},
    mfa::{MfaChallenge, MfaRepository},
    secret::Secret,
    security::TokenGenerator,
    session::{ClientInfo, Session},
    user::{User, UserId},
};
use time::OffsetDateTime;

use crate::{
    Adapters, Context, Store,
    account::load_me,
    dto::{MeDto, MfaChallengeDto, MfaMethod},
    error::AppError,
};

/// A new session. `token` goes into the cookie and is stored nowhere; only its digest is.
#[derive(Debug)]
pub struct SignedIn {
    pub me: MeDto,
    pub session: Session,
    pub token: Secret,
}

#[derive(Debug)]
pub struct MfaRequired {
    pub token: Secret,
    pub challenge: MfaChallengeDto,
}

#[derive(Debug)]
pub enum LoginOutcome {
    SignedIn(Box<SignedIn>),
    MfaRequired(MfaRequired),
}

/// Refuses disabled accounts and, when required, unverified addresses. Only called once the first
/// step checked out, so it reveals nothing to someone guessing.
pub(crate) fn ensure_can_sign_in<A: Adapters>(
    ctx: &Context<A>,
    user: &User,
) -> Result<(), AppError> {
    if user.is_disabled() {
        return Err(AppError::AccountDisabled);
    }
    if ctx.settings.require_email_verification && !user.is_email_verified() {
        return Err(AppError::EmailNotVerified);
    }
    Ok(())
}

/// The second steps the user can take, empty when two-step sign-in is off: it is on with a
/// confirmed authenticator app or any passkey. Recovery codes are offered only next to one of
/// those.
pub(crate) async fn second_factors(
    store: &mut impl Store,
    user: UserId,
) -> Result<Vec<MfaMethod>, AppError> {
    let totp = store
        .find_totp(user)
        .await?
        .is_some_and(|totp| totp.is_confirmed());
    let passkeys = store.count_user_passkeys(user).await? > 0;

    let mut methods = Vec::new();
    if totp {
        methods.push(MfaMethod::Totp);
    }
    if passkeys {
        methods.push(MfaMethod::Passkey);
    }
    if !methods.is_empty() && store.count_recovery_codes(user).await? > 0 {
        methods.push(MfaMethod::RecoveryCode);
    }
    Ok(methods)
}

/// After a first step that proved one factor (a password, an emailed or texted code, a social
/// account): starts a session, or the second step if the user has one.
///
/// `previous` is the session token the browser already had, if any; it is revoked, so signing in
/// always yields a fresh token and there is no session fixation. A pending second step is stored
/// as an `MfaChallenge` keyed by the digest of a short-lived token.
///
/// # Errors
///
/// [`AppError::AccountDisabled`], or [`AppError::EmailNotVerified`] when verification is required
/// (see [`ensure_can_sign_in`]).
pub(crate) async fn complete_first_step<A: Adapters>(
    ctx: &Context<A>,
    user: &User,
    client: ClientInfo,
    previous: Option<&Secret>,
) -> Result<LoginOutcome, AppError> {
    ensure_can_sign_in(ctx, user)?;
    let now = ctx.clock.now();
    let mut tx = ctx.db.transaction().await?;

    let methods = second_factors(&mut tx, user.id()).await?;
    if !methods.is_empty() {
        let token = ctx.tokens.generate()?;
        tx.create_mfa_challenge(&MfaChallenge::start(
            user.id(),
            ctx.tokens.digest(&token),
            now,
        ))
        .await?;
        tx.commit().await?;
        tracing::info!(user_id = %user.id(), "first sign-in step passed, second step due");
        return Ok(LoginOutcome::MfaRequired(MfaRequired {
            token,
            challenge: MfaChallengeDto { methods },
        }));
    }

    let signed_in = start_session(ctx, &mut tx, user, client, previous, now).await?;
    tx.commit().await?;
    Ok(LoginOutcome::SignedIn(Box::new(signed_in)))
}

/// Starts a session for a user who proved every factor they need: revokes `previous`, stores a new
/// session under the digest of a fresh token and returns the token once.
pub(crate) async fn start_session<A: Adapters>(
    ctx: &Context<A>,
    store: &mut impl Store,
    user: &User,
    client: ClientInfo,
    previous: Option<&Secret>,
    now: OffsetDateTime,
) -> Result<SignedIn, AppError> {
    if let Some(previous) = previous {
        store
            .delete_session_by_token(&ctx.tokens.digest(previous))
            .await?;
    }
    let token = ctx.tokens.generate()?;
    let session = Session::start(
        user.id(),
        ctx.tokens.digest(&token),
        client,
        now,
        &ctx.settings.sessions,
    );
    store.create_session(&session).await?;
    let me = load_me(store, user).await?;
    tracing::info!(user_id = %user.id(), session_id = %session.id(), "signed in");
    Ok(SignedIn { me, session, token })
}
