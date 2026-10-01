use domain::{
    audit::{AuditAction, AuditRepository, AuthMethod},
    clock::Clock,
    database::{Database, Transaction},
    i18n::Message,
    mfa::{MfaChallenge, MfaRepository},
    secret::Secret,
    security::TokenGenerator,
    session::ClientInfo,
    user::UserRepository,
};

use crate::{
    Adapters, Context,
    auth::signin::{self, SignedIn},
    error::AppError,
};

/// The one answer for an unknown, expired, used-up or over-attempted challenge.
pub(crate) fn expired() -> AppError {
    AppError::conflict("mfa_expired", Message::new("conflict-mfa-expired"))
}

/// The attempt behind the cookie, if it is still live. Does not count as an attempt, so it is for
/// reading (which methods to offer), never for checking an answer.
pub(crate) async fn load<A: Adapters>(
    ctx: &Context<A>,
    token: &Secret,
) -> Result<MfaChallenge, AppError> {
    if token.is_empty() || !token.is_within_limit() {
        return Err(expired());
    }
    ctx.db
        .connection()
        .await?
        .find_mfa_challenge(&ctx.tokens.digest(token))
        .await?
        .filter(|challenge| challenge.is_live(ctx.clock.now()))
        .ok_or_else(expired)
}

/// Counts an answer to the attempt behind the cookie before it is checked, and returns the
/// attempt if it was still live. The count is taken first and atomically, so a burst of parallel
/// guesses gets no more than [`MAX_MFA_ATTEMPTS`](domain::mfa::MAX_MFA_ATTEMPTS) evaluations.
pub(crate) async fn reserve<A: Adapters>(
    ctx: &Context<A>,
    token: &Secret,
) -> Result<MfaChallenge, AppError> {
    if token.is_empty() || !token.is_within_limit() {
        return Err(expired());
    }
    ctx.db
        .connection()
        .await?
        .reserve_mfa_attempt(&ctx.tokens.digest(token), ctx.clock.now())
        .await?
        .ok_or_else(expired)
}

/// The second step checked out: retires the attempt and starts the session. The challenge is
/// deleted first, so it completes at most once even when answers race.
pub(crate) async fn finish<A: Adapters>(
    ctx: &Context<A>,
    challenge: &MfaChallenge,
    client: ClientInfo,
    previous: Option<&Secret>,
    method: AuthMethod<'_>,
) -> Result<SignedIn, AppError> {
    let mut tx = ctx.db.transaction().await?;
    if !tx.delete_mfa_challenge(&challenge.token_hash).await? {
        return Err(expired());
    }
    let user = tx.find_user(challenge.user_id).await?.ok_or_else(expired)?;
    signin::ensure_can_sign_in(ctx, &user)?;
    let signed_in = signin::start_session(ctx, &mut tx, &user, client, previous, method).await?;
    tx.commit().await?;
    Ok(signed_in)
}

/// Records a wrong answer to the second step: the first one was right, so it may be someone who
/// knows the password.
pub(crate) async fn record_failure<A: Adapters>(
    ctx: &Context<A>,
    challenge: &MfaChallenge,
    client: &ClientInfo,
    method: AuthMethod<'_>,
) -> Result<(), AppError> {
    ctx.db
        .connection()
        .await?
        .record_audit_event(
            &ctx.event(challenge.user_id, AuditAction::SignInFailed, client)
                .detail(method.detail()),
        )
        .await?;
    Ok(())
}

/// Replaces the user's recovery codes and returns them in display form (`xxxxx-xxxxx`). Only
/// their keyed digests are stored, so this is the one time they can be shown.
pub(crate) async fn new_recovery_codes<A: Adapters>(
    ctx: &Context<A>,
    store: &mut impl MfaRepository,
    user: domain::user::UserId,
) -> Result<Vec<String>, AppError> {
    use domain::{mfa::RECOVERY_CODE_COUNT, security::Crypto};

    // 32 symbols, so every random byte maps to one without bias. No 1, i, l or o, which are easy to
    // confuse.
    const ALPHABET: &[u8; 32] = b"023456789abcdefghjkmnpqrstuvwxyz";
    let mut codes = Vec::with_capacity(RECOVERY_CODE_COUNT);
    let mut hashes = Vec::with_capacity(RECOVERY_CODE_COUNT);
    for _ in 0..RECOVERY_CODE_COUNT {
        let bytes = ctx.crypto.random_bytes(10)?;
        let raw: String = bytes
            .iter()
            .map(|byte| char::from(ALPHABET[usize::from(byte & 31)]))
            .collect();
        hashes.push(crate::codes::digest(ctx, user, &raw));
        codes.push(format!("{}-{}", &raw[..5], &raw[5..]));
    }
    store.replace_recovery_codes(user, &hashes).await?;
    Ok(codes)
}
