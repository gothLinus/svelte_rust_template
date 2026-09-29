use domain::{
    clock::Clock,
    database::Database,
    one_time_code::CodePurpose,
    session::SessionId,
    user::{User, UserId},
    user_token::TokenPurpose,
};
use time::OffsetDateTime;

use crate::{Adapters, Context, Store, error::AppError, mail, tokens};

/// Deletes every way into the account that does not need a session yet: pending email-change,
/// magic and reset links, emailed and texted codes, and half-finished sign-ins waiting for their
/// second step. After a reset or "sign out everywhere" none of them may still complete, or an
/// attacker's pending email change would outlive the owner's recovery.
pub(crate) async fn revoke_pending(store: &mut impl Store, user: UserId) -> Result<(), AppError> {
    for purpose in [
        TokenPurpose::PasswordReset,
        TokenPurpose::EmailChange,
        TokenPurpose::MagicLink,
    ] {
        store.delete_user_tokens(user, purpose).await?;
    }
    for purpose in CodePurpose::ALL {
        store.delete_one_time_code(user, purpose).await?;
    }
    store.delete_user_mfa_challenges(user).await?;
    Ok(())
}

pub(crate) async fn revoke_all(
    store: &mut impl Store,
    user: UserId,
    keep: Option<SessionId>,
) -> Result<u64, AppError> {
    let revoked = store.delete_user_sessions(user, keep).await?;
    revoke_pending(store, user).await?;
    Ok(revoked)
}

#[derive(Debug, Clone, Copy)]
pub(crate) enum Prover {
    Registrant { keep: Option<SessionId> },
    Owner,
}

#[derive(Debug)]
pub(crate) struct Proven {
    pub user: User,
    /// The first proof dropped a password the prover may not have chosen; the owner should be told
    /// how to choose one.
    pub password_removed: bool,
}

/// Records that the owner of the account's address just proved they control it (a reset,
/// verification or magic link, or an emailed code) and returns the updated user.
///
/// The first such proof also evicts whoever got in before it. Until then anyone could have
/// registered the address, or signed up through a provider that does not vouch for it, and
/// attached their own way in: so the password, linked provider accounts, passkeys, the
/// authenticator app, recovery codes, the phone number, pending links and codes, and every
/// session but `keep` are dropped. Only a [`Prover::Registrant`] keeps the password, being the one
/// who chose it. Later proofs change nothing but the timestamp.
pub(crate) async fn prove_email_ownership(
    store: &mut impl Store,
    user: UserId,
    now: OffsetDateTime,
    prover: Prover,
) -> Result<Proven, AppError> {
    let before = store
        .find_user_for_update(user)
        .await?
        .ok_or(AppError::InvalidToken)?;
    let mut password_removed = false;
    let keep = match prover {
        Prover::Registrant { keep } => keep,
        Prover::Owner => None,
    };
    if !before.is_email_verified() {
        if matches!(prover, Prover::Owner) && before.has_password() {
            store.set_user_password(user, None).await?;
            password_removed = true;
        }
        store.delete_user_identities(user).await?;
        store.delete_user_passkeys(user).await?;
        store.delete_totp(user).await?;
        store.replace_recovery_codes(user, &[]).await?;
        store.set_user_phone(user, None).await?;
        let revoked = revoke_all(store, user, keep).await?;
        tracing::info!(user_id = %user, revoked, password_removed, "first proof of address ownership");
    }
    let user = store
        .mark_user_email_verified(user, now)
        .await?
        .ok_or(AppError::InvalidToken)?;
    Ok(Proven {
        user,
        password_removed,
    })
}

/// Mails the owner a link to choose a password, after [`prove_email_ownership`] removed the one
/// the account had. Failures are logged: the proof itself succeeded.
pub(crate) async fn offer_password<A: Adapters>(ctx: &Context<A>, user: &User) {
    let issued = async {
        let mut conn = ctx.db.connection().await?;
        tokens::issue(
            ctx,
            &mut conn,
            user.id(),
            TokenPurpose::PasswordReset,
            ctx.clock.now(),
        )
        .await
    }
    .await;
    match issued {
        Ok(token) => {
            let link = ctx.settings.links.reset_password(&token);
            let ttl = ctx.settings.tokens.password_reset_ttl;
            mail::send(
                &*ctx.mailer,
                mail::choose_password(&ctx.voice(), user.email().clone(), &link, ttl),
            )
            .await;
        }
        Err(err) => {
            tracing::error!(user_id = %user.id(), error = %crate::ErrorChain(&err), "failed to offer a new password");
        }
    }
}
