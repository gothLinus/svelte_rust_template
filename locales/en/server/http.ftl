## Answers of the HTTP layer itself: requests that never reach a use case.

## The `title` of a problem document: the name of its HTTP status.

http-status-400 = Bad Request
http-status-401 = Unauthorized
http-status-403 = Forbidden
http-status-404 = Not Found
http-status-405 = Method Not Allowed
http-status-409 = Conflict
http-status-413 = Payload Too Large
http-status-415 = Unsupported Media Type
http-status-422 = Unprocessable Entity
http-status-429 = Too Many Requests
http-status-500 = Internal Server Error
http-status-502 = Bad Gateway
http-status-503 = Service Unavailable
# Any other status.
http-status-other = Error

## The `detail` of a problem document.

http-method-not-allowed = method not allowed
# Too many requests or attempts from one address or against one account.
http-rate-limited = too many attempts, try again later
http-timeout = the request took too long
# For developers of API clients. { $type } is a media type, do not translate it.
http-unsupported-media-type = expected a request with `Content-Type: { $type }`
http-invalid-body = the body is not a valid message
http-invalid-request = the request could not be read
http-payload-too-large = the request body is too large
http-invalid-query = the query string is not valid
# For developers of API clients. X-Requested-With is a header name, do not translate it.
http-csrf-header-required = state-changing requests need an X-Requested-With header
http-csrf-cross-origin = cross-origin request rejected
