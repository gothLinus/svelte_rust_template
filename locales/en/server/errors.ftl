## The `detail` of a problem document: what went wrong, for the person using the app.
## One message per kind of application error, named `error-<code>`. The machine-readable
## `code` next to it never changes with the language.

error-validation-failed = the request is invalid
error-unauthenticated = authentication required
# Deliberately vague: it does not say whether the account or the password was wrong.
error-invalid-credentials = invalid email, username or password
error-email-not-verified = verify your email address to sign in
error-account-disabled = this account has been disabled
error-forbidden = you do not have permission to do this
error-reauth-required = confirm it's you to continue
# Also the answer for things the person may not see, so it cannot be used to probe for them.
error-not-found = not found
# The client sent the version it read (`If-Match`) and the item changed since.
error-stale = this was changed by someone else in the meantime, reload it and try again
error-invalid-token = this link is invalid or has expired
error-invalid-passkey = this passkey could not be verified
error-provider-unavailable = the sign-in provider could not be reached, try again
error-busy = the server is busy, try again in a moment
# Shown for every unexpected failure; the cause is only in the server log.
error-internal = an unexpected error occurred

## Conflicts: the request is fine but clashes with the current state.
## Each has its own stable `code` (in the comment) that clients can act on.

# email_taken
conflict-email-taken = an account with this email already exists
# email_taken: the address of a pending email change was registered by someone else meanwhile.
conflict-old-address-taken = another account uses the old address now
# email_unverified
conflict-email-unverified = verify your email address first
# already_verified
conflict-already-verified = this email address is already verified
# cannot_disable_self
conflict-cannot-disable-self = you cannot disable your own account
# last_admin
conflict-last-admin = at least one enabled account must be able to manage users
# channel_unavailable. { $channel } is how the code would be sent: email, sms or whatsapp.
conflict-channel-unavailable = sending codes by { $channel ->
        [sms] text message
        [whatsapp] WhatsApp
       *[email] email
    } is not set up
# totp_enabled
conflict-totp-enabled = an authenticator app is already set up; remove it first
# totp_not_started
conflict-totp-not-started = start setting up the app first
# mfa_disabled
conflict-mfa-disabled = turn on two-step sign-in first
# mfa_expired
conflict-mfa-expired = this sign-in attempt expired, sign in again
# passkey_exists
conflict-passkey-exists = this passkey is registered already

## Signing in with an external provider (social login). { $provider } is its name, such as Google.

# oauth_cancelled
conflict-oauth-cancelled = signing in with the provider was cancelled
# email_required
conflict-oauth-email-required = the provider did not share an email address
# email_unverified
conflict-oauth-email-unverified = { $provider } did not confirm this email address; register with this address first, then link { $provider } in your security settings
# email_in_use
conflict-oauth-email-in-use = an account with this email already exists; sign in to it and link { $provider } in your security settings
# username_taken
conflict-oauth-username-taken = the generated username was just taken; please try again
# username_taken
conflict-oauth-username-unavailable = could not find a free username; please try again
# provider_linked
conflict-provider-linked = another account of this provider is linked already; unlink it first
# oauth_state_invalid
conflict-oauth-state-invalid = this sign-in attempt expired or was started in another browser, try again
# identity_taken
conflict-identity-taken = this account is linked to another user already
