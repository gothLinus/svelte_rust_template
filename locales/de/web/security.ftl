## Karten auf der Sicherheitsseite: Passwort, Passkeys, Zwei-Schritt-Anmeldung, verknüpfte Konten,
## Sitzungen, Wiederherstellungscodes.

## Passwort

security-password-title = Passwort
security-password-description = Wir senden Ihnen einen Link per E-Mail, mit dem Sie ein neues wählen. Anschließend werden alle Geräte abgemeldet.
security-password-description-none = Ihr Konto hat noch kein Passwort. Wir senden Ihnen einen Link per E-Mail, um eines festzulegen.
security-password-change = Passwort ändern
security-password-set = Passwort festlegen
# { $email } ist die Adresse des Kontos, { $minutes } gibt an, wie lange der Link gültig ist.
security-password-link-sent = Wir haben einen Link an { $email } gesendet. Er ist { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } lang gültig.

## Verknüpfte Konten

security-linked-title = Verknüpfte Konten
security-linked-description = Melden Sie sich damit statt mit Ihrem Passwort an.
security-linked-linked = Verknüpft
security-linked-not-linked = Nicht verknüpft
security-linked-link = Verknüpfen
# { $provider } ist der Name des Anbieters, z. B. Google.
security-linked-link-label = { $provider } verknüpfen
security-linked-unlink = Verknüpfung aufheben
security-linked-unlink-label = Verknüpfung mit { $provider } aufheben
security-linked-unlinked = Verknüpfung mit { $provider } aufgehoben.

## Passkeys

security-passkeys-title = Passkeys und Sicherheitsschlüssel
security-passkeys-description = Melden Sie sich mit Fingerabdruck, Gesichtserkennung, Displaysperre oder einem Hardwareschlüssel statt mit einem Passwort an.
# Wird statt der Beschreibung oben angezeigt, solange die Zwei-Schritt-Anmeldung deaktiviert ist.
security-passkeys-description-enables-two-step = Melden Sie sich mit Fingerabdruck, Gesichtserkennung, Displaysperre oder einem Hardwareschlüssel statt mit einem Passwort an. Wenn Sie einen hinzufügen, wird auch die Zwei-Schritt-Anmeldung aktiviert.
# { $added } ist ein Datum, { $used } eine relative Zeitangabe wie "vor 3 Tagen".
security-passkeys-used = Hinzugefügt am { $added } · zuletzt verwendet { $used }
security-passkeys-never-used = Hinzugefügt am { $added } · nie verwendet
security-passkeys-add = Passkey hinzufügen
security-passkeys-added = Passkey hinzugefügt.
security-passkeys-unsupported = Dieser Browser unterstützt keine Passkeys.
security-passkeys-rename = Umbenennen
# { $name } ist der Name des Passkeys.
security-passkeys-rename-label = { $name } umbenennen
security-passkeys-rename-title = Passkey umbenennen
security-passkeys-name = Name
security-passkeys-save = Speichern
security-passkeys-remove = Entfernen
security-passkeys-remove-label = { $name } entfernen
security-passkeys-remove-title = { $name } entfernen?
security-passkeys-remove-description = Sie können sich dann nicht mehr damit anmelden.
security-passkeys-remove-description-two-step = Sie können sich dann nicht mehr damit anmelden. Wenn dies Ihr einziger zweiter Schritt ist, wird die Zwei-Schritt-Anmeldung deaktiviert.
security-passkeys-removed = { $name } entfernt.

## Zwei-Schritt-Anmeldung

security-two-step-title = Zwei-Schritt-Anmeldung
security-two-step-on = Aktiv
security-two-step-off = Inaktiv
security-two-step-description = Nach Ihrem Passwort, einem per E-Mail oder SMS gesendeten Code oder einem externen Konto fragen wir zusätzlich nach einem Code aus einer Authenticator-App oder einem Passkey. Bei der Anmeldung nur mit einem Passkey entfällt dieser Schritt.

security-totp-title = Authenticator-App
# Statuszeile, wenn die App bereits eingerichtet ist.
security-totp-enabled = Eingerichtet
security-totp-hint = Google Authenticator, 1Password, Authy oder eine beliebige TOTP-App
security-totp-setup = Einrichten
security-totp-setup-label = Authenticator-App einrichten
security-totp-remove = Entfernen
security-totp-remove-label = Authenticator-App entfernen
security-totp-added = Authenticator-App hinzugefügt.
security-totp-removed = Authenticator-App entfernt.

security-totp-setup-title = Authenticator-App einrichten
security-totp-setup-description = Scannen Sie den Code mit der App oder geben Sie den Schlüssel manuell ein. Geben Sie dann den 6-stelligen Code ein, den die App anzeigt.
security-totp-qr-label = QR-Code für Ihre Authenticator-App
security-totp-key = Einrichtungsschlüssel
security-totp-copy-key = Einrichtungsschlüssel kopieren
security-totp-key-copied = Einrichtungsschlüssel kopiert.
security-totp-key-copy-failed = Kopieren fehlgeschlagen. Markieren Sie den Schlüssel und kopieren Sie ihn manuell.
security-totp-code = Code aus der App
security-totp-turn-on = Aktivieren

security-totp-remove-title = Authenticator-App entfernen?
security-totp-remove-description = Geben Sie zur Bestätigung einen aktuellen Code aus der App ein.

## Wiederherstellungscodes

security-recovery-title = Wiederherstellungscodes
# { $remaining } von { $total } Codes sind noch unbenutzt.
security-recovery-remaining = { $remaining } von { $total } übrig
security-recovery-new = Neue Codes
security-recovery-new-label = Neue Wiederherstellungscodes
security-recovery-new-title = Neue Wiederherstellungscodes erstellen?
security-recovery-new-description = Ihre aktuellen Wiederherstellungscodes werden sofort ungültig. Bewahren Sie die neuen an einem sicheren Ort auf.
security-recovery-new-confirm = Neue Codes erstellen

security-recovery-dialog-title = Wiederherstellungscodes speichern
security-recovery-dialog-description = Falls Sie Ihre Authenticator-App und Ihre Passkeys verlieren, können Sie sich mit jedem dieser Codes einmal anmelden. Bewahren Sie sie an einem sicheren Ort auf; sie werden nicht erneut angezeigt.
security-recovery-list-label = Wiederherstellungscodes
security-recovery-copy = Kopieren
security-recovery-download = Herunterladen
security-recovery-saved = Ich habe sie gespeichert
security-recovery-copied = Wiederherstellungscodes kopiert.
security-recovery-copy-failed = Kopieren fehlgeschlagen. Laden Sie sie stattdessen herunter.
# Erste Zeile der heruntergeladenen Textdatei. { $app } ist der Name der App.
security-recovery-file-title = Wiederherstellungscodes für { $app }

## Sitzungen

security-sessions-title = Aktive Sitzungen
security-sessions-description = Angemeldete Browser und Geräte. Melden Sie alle ab, die Sie nicht kennen.
security-sessions-current = Dieses Gerät
# { $when } ist eine relative Zeitangabe wie "vor 5 Minuten".
security-sessions-active = Aktiv { $when }
# { $when } ist ein Datum mit Uhrzeit.
security-sessions-signed-in = Angemeldet am { $when }
security-sessions-sign-out = Abmelden
# { $device } ist etwa "Firefox auf macOS", { $active } eine relative Zeitangabe.
security-sessions-sign-out-label = { $device } abmelden, aktiv { $active }
security-sessions-signed-out = { $device } abgemeldet.
security-sessions-everywhere = Überall abmelden
security-sessions-everywhere-title = Überall abmelden?
security-sessions-everywhere-description = Alle Browser und Geräte werden abgemeldet, auch dieses. Sie müssen sich erneut anmelden.
security-sessions-everywhere-done = Auf allen Geräten abgemeldet.

## Letzte Aktivitäten

security-activity-title = Letzte Aktivitäten
security-activity-description = Anmeldungen und Änderungen an Ihren Anmeldemethoden. Wenn Ihnen etwas unbekannt vorkommt, ändern Sie Ihr Passwort und melden Sie sich überall ab.
security-activity-empty = Noch nichts vorhanden.
security-activity-more = Mehr anzeigen
# Kennzeichnet ein Ereignis, das ein Administrator ausgelöst hat.
security-activity-by-admin = Durch einen Administrator
