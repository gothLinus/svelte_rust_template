## Files: what an upload's name, type and size must look like, and what stops one.
## Each has a stable `code` (in the comment) that clients can act on.

# too_long. { $max } is a number of characters.
file-name-too-long = must be at most { $max } characters
# invalid_characters: control characters, slashes, or a name that is only dots.
file-name-invalid = must not contain slashes or control characters
# invalid_content_type: the upload's Content-Type is not a media type such as image/png.
file-content-type-invalid = is not a valid media type, such as image/png
# too_large. { $max } is a number of megabytes (1 MB = 1,048,576 bytes).
file-too-large = must be at most { $max } MB
# file_quota_exceeded: the upload would not fit in the space the account has left.
conflict-file-quota-exceeded = there is not enough space left for this file; delete files to make room
