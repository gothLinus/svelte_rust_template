## The example resource: what a note's title and text must look like.
## `just new-resource` copies this file for a new resource, so keep every note message here.
## Each has a stable `code` (in the comment) that clients can act on.

# too_long. { $max } is a number of characters.
note-title-too-long = must be at most { $max } characters
# invalid_characters
note-title-single-line = must be a single line without control characters
# too_long. { $max } is a number of characters.
note-body-too-long = must be at most { $max } characters
# invalid_characters
note-body-control-characters = must not contain control characters
