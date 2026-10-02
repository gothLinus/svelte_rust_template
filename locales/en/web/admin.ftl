## Administration: the users page.

admin-users-title = Users
admin-users-description = Everyone with an account, newest first.
admin-search-label = Search users
admin-search-placeholder = Search by username or email
admin-empty = No users found.
# Next to the signed-in admin's own row.
admin-you = (you)
admin-disabled-badge = Disabled
# Screen-reader name of the menu button on a user's row. $name is the username.
admin-manage-label = Manage { $name }
admin-menu-roles = Roles
# Opens the audit log filtered to that user.
admin-view-activity = View activity
admin-view-sessions = View sessions
admin-sign-out = Sign out everywhere
admin-enable = Enable account
admin-disable = Disable account
# Screen-reader name of the previous/next navigation below the list.
admin-pages-label = User pages

# $name is the username.
admin-disable-title = Disable { $name }?
admin-disable-description = They are signed out on every device and cannot sign in until an admin enables the account again.
admin-disable-confirm = Disable account

# $name is the username.
admin-sign-out-title = Sign { $name } out everywhere?
admin-sign-out-description = Every session ends and pending sign-in links stop working. The account stays enabled, so they can sign in again.
admin-sign-out-confirm = Sign out everywhere

# The dialog listing someone's signed-in devices. $name is the username.
admin-sessions-title = Sessions of { $name }
admin-sessions-description = Browsers and devices signed in to this account right now.
admin-sessions-empty = Not signed in anywhere.

# Notices. $name is the username, $role the role's name.
admin-role-revoked = { $name } no longer has the { $role } role.
admin-role-granted = { $name } now has the { $role } role.
admin-enabled = { $name } can sign in again.
admin-disabled = { $name } was disabled and signed out everywhere.
admin-signed-out = { $name } was signed out everywhere.
# $device describes the browser and system, such as "Firefox on macOS".
admin-session-signed-out = { $name } was signed out on { $device }.

# $permission is the permission's technical name and stays untranslated; it is shown as code.
admin-view-only = You can view accounts. Changing roles, signing users out or disabling accounts needs the { $permission } permission.

## Audit log

admin-audit-title = Audit log
admin-audit-description = Sign-ins and security changes of every account, newest first.
admin-audit-empty = No events yet.
# Above the list when it shows one account's events. $name is the username.
admin-audit-filtered = Events of { $name }
admin-audit-filtered-unknown = Events of one account
admin-audit-show-all = Show all accounts
# Who caused an event, when that was someone else. $name is their username.
admin-audit-by = by { $name }
# Who caused an event, when their account was deleted since.
admin-audit-by-deleted = by a deleted account
# Screen-reader labels of the parts of an event's row.
admin-audit-account = Account
admin-audit-when = When
admin-audit-from = From
# Screen-reader name of the previous/next navigation below the list.
admin-audit-pages-label = Audit log pages
