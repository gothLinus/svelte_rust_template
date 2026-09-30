## The app shell: layouts, error pages and messages that belong to no single page.

# The browser tab title of a page inside the app. $page is the page's name (Notes,
# Profile, …), $site the app's name.
app-page-title = { $page } · { $site }

# The first link on every page; keyboard users use it to jump past the navigation.
app-skip-to-content = Skip to content

# Screen-reader name of the logo link that leads to the dashboard on phones. $site is the
# app's name.
app-dashboard-link = { $site } dashboard

# A notice when the server ends the session while the app is open.
app-session-ended = Your session has ended. Please sign in again.

# Heading of an error page that has no message of its own.
app-error-fallback = Something went wrong

# Shown instead of any technical detail when something unexpected broke.
app-error-unexpected = Something went wrong. Please try again.

# Development only: the API is probably not running. $command is a shell command and
# stays untranslated.
app-error-dev-hint = Is the API running? Start everything with { $command }.

app-error-retry = Try again
app-error-back-to-start = Back to the start
app-error-back-to-dashboard = Back to the dashboard

# The message of the error page for an unknown address.
not-found-message = This page does not exist.
