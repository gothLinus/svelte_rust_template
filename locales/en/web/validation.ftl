## Client-side validation of form fields. These mirror the server's rules and wording
## (`validation-*` in the server's catalog), so an error reads the same whichever side
## finds it. Each message completes the sentence started by the field's label.

validation-required = this field is required
validation-invalid-email = enter a valid email address

# { $min } is the smallest number of characters allowed.
validation-too-short = { $min ->
        [one] must be at least { $min } character
       *[other] must be at least { $min } characters
    }

# { $max } is the largest number of characters allowed.
validation-too-long = { $max ->
        [one] must be at most { $max } character
       *[other] must be at most { $max } characters
    }

validation-password-only-spaces = must not consist of spaces only
validation-username-invalid-characters = use letters, digits, dots, dashes and underscores, starting with a letter or digit
validation-username-needs-letter = must contain at least one letter

# The example is a phone number in international format.
validation-invalid-phone = enter the number with its country code, e.g. +49 170 1234567

validation-code = enter the 6-digit code
validation-control-characters = must not contain control characters
validation-single-line = must be a single line without control characters
validation-passwords-differ = the passwords do not match
