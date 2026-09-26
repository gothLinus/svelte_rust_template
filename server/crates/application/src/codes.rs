use domain::{
    clock::Clock,
    one_time_code::{CODE_DIGITS, CodeChannel, CodePurpose, OneTimeCode, OneTimeCodeRepository},
    secret::{Secret, TokenHash},
    security::Crypto,
    user::{PhoneNumber, UserId},
};

use crate::{Adapters, Context, error::AppError};

const PURPOSE: &str = "codes";

/// The stored digest of a code (six digits, or a recovery code). Keyed with the server's pepper,
/// because a million six-digit codes invert from any unkeyed hash at once; salted with the user
/// id, so equal codes of different users differ.
pub(crate) fn digest<A: Adapters>(ctx: &Context<A>, user: UserId, code: &str) -> TokenHash {
    TokenHash::new(
        ctx.crypto
            .keyed_digest(PURPOSE, format!("{user}:{code}").as_bytes()),
    )
}

pub(crate) fn previous_digest<A: Adapters>(
    ctx: &Context<A>,
    user: UserId,
    code: &str,
) -> Option<TokenHash> {
    ctx.crypto
        .previous_keyed_digest(PURPOSE, format!("{user}:{code}").as_bytes())
        .map(TokenHash::new)
}

pub(crate) fn normalize(code: &str) -> String {
    code.chars().filter(char::is_ascii_alphanumeric).collect()
}

pub(crate) async fn issue<A: Adapters>(
    ctx: &Context<A>,
    store: &mut impl OneTimeCodeRepository,
    user: UserId,
    purpose: CodePurpose,
    channel: CodeChannel,
    target: Option<PhoneNumber>,
) -> Result<Secret, AppError> {
    let code = ctx.crypto.random_digits(CODE_DIGITS)?;
    let record = OneTimeCode {
        target,
        ..OneTimeCode::issue(
            user,
            purpose,
            channel,
            digest(ctx, user, code.expose()),
            ctx.clock.now(),
        )
    };
    store.replace_one_time_code(&record).await?;
    Ok(code)
}

/// Checks a guess against the user's live code for `purpose`, sent on one of `channels`. A right
/// guess uses the code up. Every guess counts towards the attempt limit, and it is counted before
/// it is checked, so parallel guesses cannot get past the limit.
pub(crate) async fn check<A: Adapters>(
    ctx: &Context<A>,
    store: &mut impl OneTimeCodeRepository,
    user: UserId,
    purpose: CodePurpose,
    channels: &[CodeChannel],
    guess: &str,
) -> Result<OneTimeCode, AppError> {
    let stored = store
        .reserve_code_attempt(user, purpose, ctx.clock.now())
        .await?
        .filter(|code| channels.contains(&code.channel))
        .ok_or_else(AppError::invalid_code)?;

    let guess = normalize(guess);
    if guess.is_empty() || guess.len() > 16 || digest(ctx, user, &guess) != stored.code_hash {
        return Err(AppError::invalid_code());
    }
    if !store.delete_one_time_code(user, purpose).await? {
        // A concurrent request used it first.
        return Err(AppError::invalid_code());
    }
    Ok(stored)
}
