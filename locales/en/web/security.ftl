## Cards on the security page: password, passkeys, two-step sign-in, linked accounts,
## sessions, recovery codes.

## Password

security-password-title = Password
security-password-description = We email you a link to choose a new one. Every device is signed out afterwards.
security-password-description-none = Your account has no password yet. We email you a link to set one.
security-password-change = Change password
security-password-set = Set a password
# { $email } is the account's address, { $minutes } how long the link works.
security-password-link-sent = We sent a link to { $email }. It works for { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }.

## Linked accounts

security-linked-title = Linked accounts
security-linked-description = Sign in with these instead of your password.
security-linked-linked = Linked
security-linked-not-linked = Not linked
security-linked-link = Link
# { $provider } is the provider's name, e.g. Google.
security-linked-link-label = Link { $provider }
security-linked-unlink = Unlink
security-linked-unlink-label = Unlink { $provider }
security-linked-unlinked = { $provider } unlinked.

## Passkeys

security-passkeys-title = Passkeys and security keys
security-passkeys-description = Sign in with your fingerprint, face, screen lock or a hardware key instead of a password.
# Shown instead of the description above while two-step sign-in is off.
security-passkeys-description-enables-two-step = Sign in with your fingerprint, face, screen lock or a hardware key instead of a password. Adding one also turns on two-step sign-in.
# { $added } is a date, { $used } a relative time like "3 days ago".
security-passkeys-used = Added { $added } · used { $used }
security-passkeys-never-used = Added { $added } · never used
security-passkeys-add = Add a passkey
security-passkeys-added = Passkey added.
security-passkeys-unsupported = This browser does not support passkeys.
# { $name } is the passkey's name.
security-passkeys-rename = Rename
security-passkeys-rename-label = Rename { $name }
security-passkeys-rename-title = Rename passkey
security-passkeys-name = Name
security-passkeys-save = Save
security-passkeys-remove = Remove
security-passkeys-remove-label = Remove { $name }
security-passkeys-remove-title = Remove { $name }?
security-passkeys-remove-description = You can no longer sign in with it.
security-passkeys-remove-description-two-step = You can no longer sign in with it. If it is your only second step, two-step sign-in turns off.
security-passkeys-removed = Removed { $name }.

## Two-step sign-in

security-two-step-title = Two-step sign-in
security-two-step-on = On
security-two-step-off = Off
security-two-step-description = After your password, an emailed or texted code, or a social account, we also ask for a code from an authenticator app or a passkey. Signing in with a passkey alone skips this step.

security-totp-title = Authenticator app
# Status line when the app is already set up.
security-totp-enabled = Set up
security-totp-hint = Google Authenticator, 1Password, Authy, or any TOTP app
security-totp-setup = Set up
security-totp-setup-label = Set up authenticator app
security-totp-remove = Remove
security-totp-remove-label = Remove authenticator app
security-totp-added = Authenticator app added.
security-totp-removed = Authenticator app removed.

security-totp-setup-title = Set up an authenticator app
security-totp-setup-description = Scan the code with the app, or enter the key by hand. Then type the 6-digit code it shows.
security-totp-qr-label = QR code for your authenticator app
security-totp-key = Setup key
security-totp-copy-key = Copy setup key
security-totp-key-copied = Setup key copied.
security-totp-key-copy-failed = Could not copy. Select the key and copy it by hand.
security-totp-code = Code from the app
security-totp-turn-on = Turn on

security-totp-remove-title = Remove the authenticator app?
security-totp-remove-description = Enter a current code from the app to confirm.

## Recovery codes

security-recovery-title = Recovery codes
# { $remaining } of { $total } codes are still unused.
security-recovery-remaining = { $remaining } of { $total } left
security-recovery-new = New codes
security-recovery-new-label = New recovery codes
security-recovery-new-title = Create new recovery codes?
security-recovery-new-description = Your current recovery codes stop working at once. Save the new ones somewhere safe.
security-recovery-new-confirm = Create new codes

security-recovery-dialog-title = Save your recovery codes
security-recovery-dialog-description = If you lose your authenticator app and passkeys, each of these codes signs you in once. Keep them somewhere safe; they are not shown again.
security-recovery-list-label = Recovery codes
security-recovery-copy = Copy
security-recovery-download = Download
security-recovery-saved = I saved them
security-recovery-copied = Recovery codes copied.
security-recovery-copy-failed = Could not copy. Download them instead.
# First line of the downloaded text file. { $app } is the app's name.
security-recovery-file-title = { $app } recovery codes

## Sessions

security-sessions-title = Active sessions
security-sessions-description = Signed-in browsers and devices. Sign out any you do not recognise.
security-sessions-current = This device
# { $when } is a relative time like "5 minutes ago".
security-sessions-active = Active { $when }
# { $when } is a date and time.
security-sessions-signed-in = Signed in { $when }
security-sessions-sign-out = Sign out
# { $device } is like "Firefox on macOS", { $active } a relative time.
security-sessions-sign-out-label = Sign out { $device }, active { $active }
security-sessions-signed-out = Signed out { $device }.
security-sessions-everywhere = Sign out everywhere
security-sessions-everywhere-title = Sign out everywhere?
security-sessions-everywhere-description = Every browser and device is signed out, this one included. You need to sign in again.
security-sessions-everywhere-done = Signed out on every device.
