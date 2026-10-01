## Files: uploads kept in the object store, their page and the rename dialog.

# The page's name in the sidebar, the tab bar and the account menu.
files-nav = Files

files-title = Files
files-description = Your uploads, newest first.
files-upload = Upload
# Screen-reader name of the hidden file picker the Upload button opens.
files-upload-label = Choose files to upload

# Screen-reader name of the group of buttons that switches between one's own and all files.
files-scope-label = Whose files
files-scope-mine = My files
files-scope-all = Everyone's files

# Above the list, when the account has a quota. $used and $quota are sizes such as "1.5 MB".
files-usage = { $used } of { $quota } used
# Screen-reader name of the bar that shows how full the storage is.
files-usage-label = Storage used

files-empty-title = No files yet
files-empty-description = Upload a file to keep it here.
files-upload-first = Upload a file

# The name a file is uploaded under when nothing of its own name is usable.
files-unnamed = unnamed

# While a file is on its way. $name is the file's name.
files-uploading = Uploading “{ $name }”…

# On a file's row. $size is a size such as "1.5 MB", $type a media type such as image/png, $when a relative time such as "5 minutes ago".
files-details = { $size } · { $type } · { $when }
# Badge on a file somebody else uploaded (shown to those who may see everyone's files).
files-others = Another user's

files-download = Download
files-rename = Rename
files-delete = Delete
# Screen-reader names of the buttons on a file's row. $name is the file's name.
files-download-label = Download { $name }
files-rename-label = Rename { $name }
files-delete-label = Delete { $name }

# Screen-reader name of the previous/next navigation below the list.
files-pages-label = Files pages

files-delete-title = Delete this file?
files-delete-description = “{ $name }” will be gone for good.
files-delete-confirm = Delete

# Notices. $name is the file's name; $max a size such as "25 MB".
files-uploaded = Uploaded “{ $name }”.
files-too-large = “{ $name }” is larger than { $max }.
# When the server refused an upload. $reason is its reason, such as "there is not enough space left for this file".
files-upload-failed = “{ $name }” was not uploaded: { $reason }
files-renamed = Renamed to “{ $name }”.
files-deleted = Deleted “{ $name }”.

## The dialog to rename a file.

file-rename-title = Rename file
file-rename-description = The contents stay as they are.
file-field-name = Name
file-rename-save = Rename
# Shown under the name field; the server says the same (`file-name-invalid` in its catalog).
file-name-invalid = must not contain slashes or control characters
