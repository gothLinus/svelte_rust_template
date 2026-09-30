## Emails. Plain text, one subject and one body per mail, named
## `mail-<template>-subject` and `mail-<template>-body`.
##
## Placeholders:
##   { $link }     a full URL; keep it on a line of its own
##   { $code }     the one-time code, such as "123 456"; keep it on a line of its own
##   { $hours }    how many hours the link works
##   { $minutes }  how many minutes the link or code works
##   { $email }    an email address
##   { $what }     one sentence from the `notice-` messages below
## A paragraph is one line; a blank line separates paragraphs.

mail-verify-email-subject = Verify your email address
mail-verify-email-body =
    Welcome! Please confirm that this is your email address by opening this link within { $hours ->
        [one] { $hours } hour
       *[other] { $hours } hours
    }:

    { $link }

    If you did not create an account, you can ignore this email.

mail-password-reset-subject = Reset your password
mail-password-reset-body =
    Someone, hopefully you, asked to reset the password of your account.

    Open this link within { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    } to choose a new password:

    { $link }

    If you did not ask for this, ignore this email. Your password stays unchanged.

# Sent after the first proof of the address removed a password the owner may not have chosen.
mail-choose-password-subject = Choose a password
mail-choose-password-body =
    Your email address is confirmed. The account was registered from another browser, so to keep out whoever else might have set it up, its password was removed and other devices were signed out.

    Open this link within { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    } to choose a password:

    { $link }

    You can also sign in with a code sent to this address, and ask for a new link from the sign-in page at any time.

mail-password-changed-subject = Your password was changed
# { $link } starts a password reset.
mail-password-changed-body =
    The password of your account was just changed, and your other devices were signed out.

    If this was not you, reset your password right away:

    { $link }

# The code is on a line of its own, so it is easy to read off and to copy.
mail-sign-in-code-subject = Your sign-in code
mail-sign-in-code-body =
    Use this code to sign in:

    { $code }

    Or open this link on the device you want to sign in on:

    { $link }

    Both work once, for { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }. If you did not try to sign in, you can ignore this email.

# Sent when someone tries to register an address that already has an account.
# { $link } starts a password reset.
mail-already-registered-subject = You already have an account
mail-already-registered-body =
    Someone, hopefully you, tried to create an account with this email address, but it already has one. Sign in instead, or reset your password if you forgot it:

    { $link }

    If this was not you, you can ignore this email.

# The code is on a line of its own, so it is easy to read off and to copy.
mail-reauthentication-code-subject = Confirm it's you
mail-reauthentication-code-body =
    Enter this code to confirm a change to your account:

    { $code }

    It works once, for { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }. If you did not just try to change your account, someone else is signed in to it: reset your password and sign out every other device.

# Sent to the old address. { $email } is the new one (partly hidden); { $link } cancels the change.
mail-email-change-requested-subject = Your email address is about to change
mail-email-change-requested-body =
    Someone asked to change the email address of your account to { $email }. The change happens once the new address is confirmed.

    If this was not you, cancel it, which also signs out every device:

    { $link }

    Then reset your password.

# { $link } starts a password reset.
mail-email-change-cancelled-subject = The email change was cancelled
mail-email-change-cancelled-body =
    Your account keeps this email address, and every device was signed out.

    Someone else may know your password: choose a new one now.

    { $link }

# Sent to the new address.
mail-confirm-email-change-subject = Confirm your new email address
mail-confirm-email-change-body =
    Open this link within { $hours ->
        [one] { $hours } hour
       *[other] { $hours } hours
    } to make this the email address of your account:

    { $link }

    If you did not ask for this, ignore this email. Nothing changes.

# Sent about a change to the account's security settings. { $what } is one of the
# `notice-` sentences; { $link } opens the security settings.
mail-security-notice-subject = A security setting of your account changed
mail-security-notice-body =
    { $what }

    If this was you, there is nothing to do. If it was not, sign in, review your security settings and sign out every other device:

    { $link }

## The sentences a security notice starts with (`{ $what }` above).

# { $provider } is the name of the service, such as Google.
notice-identity-linked = Your { $provider } account was linked to your account and can now be used to sign in.
# { $email } is the new address, partly hidden.
notice-email-changed = The email address of your account was changed to { $email }.
# { $name } is the name the person gave the passkey.
notice-passkey-added = A passkey ({ $name }) was added to your account.
notice-passkey-removed = A passkey was removed from your account.
notice-totp-added = An authenticator app was added to your account.
notice-totp-removed = The authenticator app was removed from your account.
