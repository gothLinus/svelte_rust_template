## Components of the sign-in and sign-up pages: inputs and buttons shared by them.

# Example of a one-time code in the input (six digits, one space in the middle).
signin-code-placeholder = 123 456

# Screen-reader name of the button that reveals the password; it stays the same when pressed.
signin-show-password = Show password

# The word between two ways to sign in, on a divider line.
signin-or = or

# Label of the choice between texting a code by SMS or by WhatsApp.
signin-channel-label = Send the code by
signin-channel-sms = SMS
signin-channel-whatsapp = WhatsApp
signin-channel-text = Text message

## Why a social sign-in came back with an error. The id ends with the server's code,
## `_` written as `-`.

signin-oauth-error-oauth-cancelled = Signing in with the provider was cancelled.
signin-oauth-error-oauth-state-invalid = That sign-in attempt expired or was started in another browser. Please try again.
signin-oauth-error-email-in-use = An account with this email already exists. Sign in to it, then link the provider under Settings → Security.
signin-oauth-error-email-required = The provider did not share an email address with us.
signin-oauth-error-provider-unavailable = The provider could not be reached. Please try again.
signin-oauth-error-identity-taken = That account is already linked to another user.
signin-oauth-error-provider-linked = Another account of this provider is linked already. Unlink it first.
signin-oauth-error-account-disabled = This account has been disabled.
signin-oauth-error-email-not-verified = Verify your email address to sign in. We sent you a link.
signin-oauth-error-email-unverified = The provider did not confirm this email address. Sign in another way, or verify your address first.
signin-oauth-error-reauth-required = Confirm it is you, then link the account again.
signin-oauth-error-rate-limited = Too many attempts. Please wait a moment and try again.
signin-oauth-error-not-found = This sign-in provider is not available.
signin-oauth-error-unauthenticated = Sign in first to link an account.

# Any other code.
signin-oauth-error-failed = Signing in with the provider failed. Please try again.

## Session

# Banner shown when the app could not ask the server whether anyone is signed in.
session-error-title = We could not check whether you are signed in
session-error-retry = Try again

## Devices

# The label of a signed-in device. { $browser } and { $system } are names like Firefox and macOS.
session-device = { $browser } on { $system }
session-device-unknown = Unknown device

## Email verification banner

email-verification-title = Please verify your email address
# { $email } is the address that was sent a link.
email-verification-body = We sent a link to { $email }. Open it to confirm the address is yours.
email-verification-resend = Send the link again
email-verification-resent = We sent a new link to { $email }.
