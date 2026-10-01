## Messages next to a form field the person filled in wrongly. They finish the field's
## label, so they start in lower case: "Username: must be at least 3 characters".
## Each has a stable `code` (in the comment) that clients can act on.

# required
validation-required = this field is required

## Sign-in identifiers

# invalid_email
validation-invalid-email = enter a valid email address
# invalid_phone. The example is a number with its country code.
validation-invalid-phone = enter the number with its country code, e.g. +49 170 1234567
# invalid_calling_code. { $code } is what was entered.
validation-invalid-calling-code = `{ $code }` is not a calling code such as +49
# phone_taken
validation-phone-taken = another account uses this number
# phone_country_unsupported
validation-phone-country-unsupported = numbers from this country are not supported
# unchanged
validation-email-unchanged = this is your current address

## Languages.

# unsupported_locale
validation-locale-unsupported = not a language this app is available in

## Usernames. { $min } and { $max } are numbers of characters.

# too_short
validation-username-too-short = must be at least { $min } characters
# too_long
validation-username-too-long = must be at most { $max } characters
# invalid_username
validation-username-invalid-characters = use letters, digits, dots, dashes and underscores, starting with a letter or digit
# invalid_username
validation-username-needs-letter = must contain at least one letter
# username_taken
validation-username-taken = this username is taken

## Passwords. { $min } and { $max } are numbers of characters.

# too_short
validation-password-too-short = must be at least { $min } characters
# too_long
validation-password-too-long = must be at most { $max } characters
# too_weak
validation-password-only-spaces = must not consist of spaces only
# too_common
validation-password-too-common = this password is among the most common ones; choose another
# incorrect_password
validation-incorrect-password = the password is incorrect
# invalid_credential
validation-invalid-credential = this is not correct

## Codes sent to the person

# invalid_code: a code from an email, text or authenticator app, or a recovery code.
validation-invalid-code = this code is invalid or has expired

## Passkeys. { $max } is a number of characters.

# too_long
validation-passkey-name-too-long = must be at most { $max } characters
# invalid_characters
validation-passkey-name-control-characters = must not contain control characters

## Lists

# out_of_range. Page sizes: { $min } and { $max } are numbers of items.
validation-page-size-out-of-range = must be between { $min } and { $max }
# invalid_cursor
validation-invalid-cursor = not a valid page cursor
# too_long. { $max } is a number of characters.
validation-search-too-long = must be at most { $max } characters
# too_short. { $min } is a number of characters.
validation-search-too-short = must be at least { $min } characters

## Values of the API that no person types; they show up when a client sends nonsense.

# invalid
validation-invalid-uuid = must be a UUID
# invalid
validation-not-allowed-value = is not one of the allowed values
# invalid_id
validation-invalid-id = not a valid id
# invalid_role
validation-invalid-role = not a valid role name
# unknown_permission. { $permission } is the name that was sent.
validation-unknown-permission = unknown permission `{ $permission }`
