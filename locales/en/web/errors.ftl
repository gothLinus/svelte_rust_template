## Errors found in the browser. Errors the server answers with are worded by the server, in
## the language the app sends in `Accept-Language`.

error-network = Could not reach the server. Check your connection and try again.
error-timeout = The server took too long to answer. Please try again.
error-aborted = The request was cancelled.
error-invalid-response = The server sent an answer we could not read.
error-unknown = Something went wrong. Please try again.

# A failed response that is not a problem document and has no status text (e.g. from a proxy).
# { $status } is the HTTP status code.
error-request-failed = Request failed with status { $status }.

# Replaces the server's wording for two codes where it does not say what to do next.
error-email-unverified = Verify your email address first: open the link we sent you, or send a new one from the notice at the top of the page.
error-reauth-required = Confirm it is you to make this change.

# Shown on the error page when the user opens a page without the needed permission.
error-forbidden = You do not have permission to see this page.

# A field the form has no input for was rejected by the server.
# { $field } is the field's name, e.g. "New password"; { $message } completes the sentence.
form-field-problem = { $field } { $message }.
