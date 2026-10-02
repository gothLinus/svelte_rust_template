## Verwaltung: die Benutzerseite.

admin-users-title = Benutzer
admin-users-description = Alle mit einem Konto, die neuesten zuerst.
admin-search-label = Benutzer suchen
admin-search-placeholder = Nach Benutzername oder E-Mail-Adresse suchen
admin-empty = Keine Benutzer gefunden.
# Neben der eigenen Zeile des angemeldeten Administrators.
admin-you = (Sie)
admin-disabled-badge = Deaktiviert
# Screenreader-Name der Menüschaltfläche in der Zeile eines Benutzers. $name ist der Benutzername.
admin-manage-label = { $name } verwalten
admin-menu-roles = Rollen
# Öffnet das Audit-Log, gefiltert auf diesen Benutzer.
admin-view-activity = Aktivität anzeigen
admin-view-sessions = Sitzungen anzeigen
admin-sign-out = Überall abmelden
admin-enable = Konto aktivieren
admin-disable = Konto deaktivieren
# Screenreader-Name der Zurück/Weiter-Navigation unter der Liste.
admin-pages-label = Benutzerseiten

# $name ist der Benutzername.
admin-disable-title = { $name } deaktivieren?
admin-disable-description = Die Person wird auf allen Geräten abgemeldet und kann sich erst wieder anmelden, wenn ein Administrator das Konto wieder aktiviert.
admin-disable-confirm = Konto deaktivieren

# $name ist der Benutzername.
admin-sign-out-title = { $name } überall abmelden?
admin-sign-out-description = Alle Sitzungen enden und ausstehende Anmeldelinks werden ungültig. Das Konto bleibt aktiv, die Person kann sich also wieder anmelden.
admin-sign-out-confirm = Überall abmelden

# Der Dialog mit den angemeldeten Geräten einer Person. $name ist der Benutzername.
admin-sessions-title = Sitzungen von { $name }
admin-sessions-description = Browser und Geräte, die gerade bei diesem Konto angemeldet sind.
admin-sessions-empty = Nirgends angemeldet.

# Hinweise. $name ist der Benutzername, $role der Name der Rolle.
admin-role-revoked = { $name } hat die Rolle { $role } nicht mehr.
admin-role-granted = { $name } hat jetzt die Rolle { $role }.
admin-enabled = { $name } kann sich wieder anmelden.
admin-disabled = { $name } wurde deaktiviert und überall abgemeldet.
admin-signed-out = { $name } wurde überall abgemeldet.
# $device beschreibt Browser und System, etwa „Firefox unter macOS“.
admin-session-signed-out = { $name } wurde auf { $device } abgemeldet.

# $permission ist der technische Name der Berechtigung und bleibt unübersetzt; er wird als Code angezeigt.
admin-view-only = Sie können Konten ansehen. Um Rollen zu ändern, Benutzer abzumelden oder Konten zu deaktivieren, benötigen Sie die Berechtigung { $permission }.

## Audit-Log

admin-audit-title = Audit-Log
admin-audit-description = Anmeldungen und Sicherheitsänderungen aller Konten, die neuesten zuerst.
admin-audit-empty = Noch keine Ereignisse.
# Über der Liste, wenn sie die Ereignisse eines Kontos zeigt. $name ist der Benutzername.
admin-audit-filtered = Ereignisse von { $name }
admin-audit-filtered-unknown = Ereignisse eines Kontos
admin-audit-show-all = Alle Konten anzeigen
# Wer ein Ereignis ausgelöst hat, wenn das jemand anderes war. $name ist dessen Benutzername.
admin-audit-by = durch { $name }
# Wer ein Ereignis ausgelöst hat, wenn dessen Konto inzwischen gelöscht wurde.
admin-audit-by-deleted = durch ein gelöschtes Konto
# Screenreader-Beschriftungen der Teile einer Ereigniszeile.
admin-audit-account = Konto
admin-audit-when = Zeitpunkt
admin-audit-from = Herkunft
# Screenreader-Name der Zurück/Weiter-Navigation unter der Liste.
admin-audit-pages-label = Audit-Log-Seiten
