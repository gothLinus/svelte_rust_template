use base64::{Engine, engine::general_purpose::URL_SAFE_NO_PAD};
use domain::{
    secret::{Secret, constant_time_eq},
    security::{Crypto, TokenGenerator},
    user::UserId,
    user_token::{TokenPurpose, UserToken},
};
use time::OffsetDateTime;

use crate::{Adapters, Context, Store, error::AppError};

const REGISTRATION: &str = "registration";

pub(crate) async fn issue<A: Adapters>(
    ctx: &Context<A>,
    store: &mut impl Store,
    user: UserId,
    purpose: TokenPurpose,
    now: OffsetDateTime,
) -> Result<Secret, AppError> {
    let token = ctx.tokens.generate()?;
    let record = UserToken::issue(
        user,
        purpose,
        ctx.tokens.digest(&token),
        now,
        &ctx.settings.tokens,
    );
    store.replace_user_token(&record).await?;
    Ok(token)
}

/// A mark for the browser that just registered `user`, kept in a cookie until the verification
/// link expires. Opening the link in the same browser keeps the password chosen at registration;
/// anywhere else the password may be a stranger's (see
/// [`crate::auth::access::prove_email_ownership`]).
pub(crate) fn registration_mark<A: Adapters>(
    ctx: &Context<A>,
    user: UserId,
    now: OffsetDateTime,
) -> Secret {
    browser_mark(
        ctx,
        REGISTRATION,
        user,
        now + ctx.settings.tokens.email_verification_ttl,
    )
}

/// A mark that looks like [`registration_mark`] but matches no account, so registering a taken
/// address answers exactly like registering a new one.
pub(crate) fn decoy_registration_mark<A: Adapters>(
    ctx: &Context<A>,
    now: OffsetDateTime,
) -> Result<Secret, AppError> {
    let expires = (now + ctx.settings.tokens.email_verification_ttl).unix_timestamp();
    let random = ctx.crypto.random_bytes(32)?;
    Ok(Secret::new(format!(
        "{expires}.{}",
        URL_SAFE_NO_PAD.encode(random)
    )))
}

pub(crate) fn is_registration_mark<A: Adapters>(
    ctx: &Context<A>,
    user: UserId,
    mark: &Secret,
    now: OffsetDateTime,
) -> bool {
    is_browser_mark(ctx, REGISTRATION, user, mark, now)
}

/// A mark for a browser `user` signed in on, kept for [`KNOWN_DEVICE_TTL`]. Password sign-ins
/// from it are exempt from the account-wide ceiling on failed attempts, so strangers guessing
/// the password cannot lock the owner out (OWASP's "device cookies").
pub(crate) fn device_mark<A: Adapters>(
    ctx: &Context<A>,
    user: UserId,
    now: OffsetDateTime,
) -> Secret {
    browser_mark(ctx, DEVICE, user, now + KNOWN_DEVICE_TTL)
}

pub(crate) fn is_device_mark<A: Adapters>(
    ctx: &Context<A>,
    user: UserId,
    mark: &Secret,
    now: OffsetDateTime,
) -> bool {
    is_browser_mark(ctx, DEVICE, user, mark, now)
}

pub const KNOWN_DEVICE_TTL: time::Duration = time::Duration::days(180);

const DEVICE: &str = "known device";

/// `<expiry>.<keyed digest of purpose, user and expiry>`. Nothing is stored and only the
/// server's key makes a valid mark; the user id is not in it, so a mark reveals nothing about the
/// account.
fn browser_mark<A: Adapters>(
    ctx: &Context<A>,
    purpose: &str,
    user: UserId,
    expires: OffsetDateTime,
) -> Secret {
    let expires = expires.unix_timestamp();
    let digest = ctx
        .crypto
        .keyed_digest(purpose, format!("{user}:{expires}").as_bytes());
    Secret::new(format!("{expires}.{}", URL_SAFE_NO_PAD.encode(digest)))
}

fn is_browser_mark<A: Adapters>(
    ctx: &Context<A>,
    purpose: &str,
    user: UserId,
    mark: &Secret,
    now: OffsetDateTime,
) -> bool {
    if !mark.is_within_limit() {
        return false;
    }
    let Some((expires, digest)) = mark.expose().split_once('.') else {
        return false;
    };
    let Ok(expires) = expires.parse::<i64>() else {
        return false;
    };
    if expires <= now.unix_timestamp() {
        return false;
    }
    let Ok(digest) = URL_SAFE_NO_PAD.decode(digest) else {
        return false;
    };
    let expected = ctx
        .crypto
        .keyed_digest(purpose, format!("{user}:{expires}").as_bytes());
    constant_time_eq(&digest, &expected)
}
