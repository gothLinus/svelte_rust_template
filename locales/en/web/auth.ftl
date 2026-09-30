## Signed-out pages: sign-in, registration, password reset and the pages that emailed links
## open. Every page has its own messages even where the English is the same, because the
## context differs. "Site" is always the app's name, which comes from the server.

## Sign in

# Browser tab titles: $site is the app's name.
login-page-title = Sign in · { $site }
login-title = Welcome back
login-description = Sign in to your account.
login-identifier = Email or username
login-password = Password
login-forgot = Forgot your password?
login-submit = Sign in
login-by-passkey = Sign in with a passkey
login-by-email = Email me a sign-in code
login-by-text = Text me a sign-in code
# A question followed by the link that answers it: "No account yet? Create one".
login-no-account = No account yet?
login-create-account = Create one

## Sign in with a code sent by email

login-email-page-title = Sign in with a code · { $site }
login-email-title = Sign in with a code
# After sending: $email is the address the person typed. It is shown emphasized.
# $minutes is how long the code and the link work.
login-email-sent = If an account uses { $email }, we emailed it a 6-digit code and a sign-in link. Both work once, for { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }.
login-email-description = We will email you a code and a link. No password needed.
login-email-label = Email
login-email-send = Email me a code
login-email-code = Code
login-email-submit = Sign in
login-email-another = Use another address
login-email-other-ways = Other ways to sign in

## Sign in with a code sent by text message

phone-page-title = Sign in with your phone · { $site }
phone-title = Sign in with your phone
# After sending: $phone is the number the person typed. It is shown emphasized.
# $minutes is how long the code works.
phone-sent = If an account has verified { $phone }, we sent it a 6-digit code. It works once, for { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }.
phone-description = For accounts with a verified phone number. We will send you a code.
phone-unavailable = Signing in by text message is not available.
phone-label = Phone number
# An example number; use one in your region's format.
phone-placeholder = +49 170 1234567
phone-send = Send code
phone-code = Code
phone-submit = Sign in
phone-another = Use another number
phone-other-ways = Other ways to sign in

## Second step of sign-in

mfa-page-title = Two-step sign-in · { $site }
mfa-title = Confirm it's you
mfa-description-pending = Your account uses two-step sign-in.
mfa-description-passkey = Use your passkey or security key.
mfa-description-totp = Enter the 6-digit code from your authenticator app.
mfa-description-recovery = Enter one of the recovery codes you saved when you turned on two-step sign-in.
# Followed by a link to the sign-in page (mfa-expired-link).
mfa-expired = This sign-in attempt expired.
mfa-expired-link = Sign in again
mfa-passkey = Use a passkey or security key
mfa-label-totp = Authentication code
mfa-label-recovery = Recovery code
# The shape of a recovery code, in an empty field.
mfa-recovery-placeholder = xxxxx-xxxxx
mfa-submit = Continue
mfa-use-totp = Use your authenticator app
mfa-back = Back
mfa-use-recovery = Use a recovery code

## Sign in with the link from an email

magic-page-title = Sign in · { $site }
magic-incomplete = This link is incomplete.
magic-failed-title = We could not sign you in
# $reason is a sentence that says what went wrong, from the server; $minutes is how long
# sign-in links work.
magic-failed-description = { $reason } Sign-in links work once and expire after { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }.
magic-title = Sign in to { $site }
magic-description = You opened a sign-in link from your email.
magic-retry = Send a new link
magic-continue = Continue

## Registration

register-page-title = Create account · { $site }
register-title = Create an account
register-description = It takes less than a minute.
register-username = Username
register-username-hint = Sign in with it or your email.
register-email = Email
register-password = Password
register-confirmation = Confirm password
register-submit = Create account
# A question followed by the link that answers it: "Already have an account? Sign in".
register-have-account = Already have an account?
register-sign-in = Sign in

## Registration, when the address must be verified first

register-pending-title = Check your inbox
# $email is the address the person typed. It is shown emphasized.
register-pending-description = We sent a link to { $email }. Open it to verify your address, then sign in.
register-pending-continue = Go to sign in

## Forgot password

forgot-page-title = Reset password · { $site }
forgot-title = Forgot your password?
forgot-description = Enter your email and we will send you a link to reset it.
forgot-email = Email
forgot-submit = Send reset link
# A question followed by the link that answers it: "Remembered it? Sign in".
forgot-remembered = Remembered it?
forgot-sign-in = Sign in
forgot-sent-title = Check your inbox
# The same answer whether or not the address has an account. $email is shown emphasized;
# $minutes is how long the link works.
forgot-sent-description = If an account uses { $email }, we sent it a link to choose a new password. The link works once and expires in { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }.
forgot-sent-back = Back to sign in

## Choose a new password (the link from the reset email)

reset-page-title = Choose a new password · { $site }
reset-title = Choose a new password
reset-description = You will be signed out on every device.
# Followed by a link to request a new one (reset-request-new).
reset-incomplete = This link is incomplete. Open the link from the email again.
# Shown after an error, followed by a link (reset-request-new). $minutes is how long
# reset links work.
reset-expiry = Links work once and expire after { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }.
reset-request-new = Request a new link
reset-password = New password
reset-confirmation = Confirm new password
reset-submit = Set new password
reset-done = Your password was changed. Sign in with the new one.

## Verify the address after registering (the link from the email)

verify-page-title = Verify email · { $site }
verify-incomplete = This link is incomplete.
verify-verifying = Verifying your email…
verify-done-title = Email verified
verify-done-description = Thanks! Your address is confirmed.
verify-failed-title = We could not verify your email
# $reason is a sentence that says what went wrong, from the server; $hours is how long
# verification links work.
verify-failed-description = { $reason } Links work once and expire after { $hours ->
        [one] { $hours } hour
       *[other] { $hours } hours
    }; you can request a new one from the app.
verify-continue = Continue to the app
verify-sign-in = Sign in

## Confirm a new address (the link sent to it)

confirm-page-title = Confirm email · { $site }
confirm-incomplete = This link is incomplete.
confirm-confirming = Confirming your new address…
confirm-done-title = Email address changed
confirm-done-description = Use your new address to sign in from now on.
confirm-failed-title = We could not change your email
# $reason is a sentence that says what went wrong, from the server; $hours is how long
# confirmation links work.
confirm-failed-description = { $reason } Links work once and expire after { $hours ->
        [one] { $hours } hour
       *[other] { $hours } hours
    }; request a new one in your profile.
confirm-profile = Back to your profile
confirm-sign-in = Sign in

## Cancel an address change (the link sent to the old address)

cancel-page-title = Cancel email change · { $site }
cancel-incomplete = This link is incomplete.
cancel-title = Cancel the email change?
cancel-description = Someone asked to move your account to another address. Cancelling keeps this one and signs out every device.
cancel-submit = Cancel the change
cancel-done-title = Email change cancelled
cancel-done-description = Your account keeps this address, and every device was signed out. If you did not ask for the change, someone may know your password: choose a new one.
cancel-new-password = Choose a new password
cancel-failed-title = We could not cancel the change
cancel-sign-in = Sign in
