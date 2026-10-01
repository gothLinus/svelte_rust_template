## Seiten ohne Anmeldung: Anmeldung, Registrierung, Zurücksetzen des Passworts und die Seiten, die
## Links aus E-Mails öffnen. Jede Seite hat eigene Nachrichten, auch wo der englische Text gleich ist,
## weil der Kontext ein anderer ist. "Site" ist immer der Name der App, der vom Server kommt.

## Anmelden

# Titel im Browser-Tab: $site ist der Name der App.
login-page-title = Anmelden · { $site }
login-title = Willkommen zurück
login-description = Melden Sie sich bei Ihrem Konto an.
login-identifier = E-Mail-Adresse oder Benutzername
login-password = Passwort
login-forgot = Passwort vergessen?
login-submit = Anmelden
login-by-passkey = Mit Passkey anmelden
login-by-email = Anmeldecode per E-Mail senden
login-by-text = Anmeldecode per SMS senden
# Eine Frage, gefolgt vom Link, der sie beantwortet: "Noch kein Konto? Konto erstellen".
login-no-account = Noch kein Konto?
login-create-account = Konto erstellen

## Anmeldung mit einem per E-Mail gesendeten Code

login-email-page-title = Mit Code anmelden · { $site }
login-email-title = Mit Code anmelden
# Nach dem Senden: $email ist die eingegebene Adresse. Sie wird hervorgehoben angezeigt.
# $minutes gibt an, wie lange Code und Link gültig sind.
login-email-sent = Falls ein Konto { $email } verwendet, haben wir einen 6-stelligen Code und einen Anmeldelink dorthin gesendet. Beide sind einmalig und { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } lang gültig.
login-email-description = Wir senden Ihnen einen Code und einen Link per E-Mail. Kein Passwort nötig.
login-email-label = E-Mail-Adresse
login-email-send = Code per E-Mail senden
login-email-code = Code
login-email-submit = Anmelden
login-email-another = Andere Adresse verwenden
login-email-other-ways = Andere Anmeldemethoden

## Anmeldung mit einem per SMS gesendeten Code

phone-page-title = Mit Telefonnummer anmelden · { $site }
phone-title = Mit Telefonnummer anmelden
# Nach dem Senden: $phone ist die eingegebene Nummer. Sie wird hervorgehoben angezeigt.
# $minutes gibt an, wie lange der Code gültig ist.
phone-sent = Falls { $phone } bei einem Konto bestätigt ist, haben wir einen 6-stelligen Code an diese Nummer gesendet. Er ist einmalig und { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } lang gültig.
phone-description = Für Konten mit bestätigter Telefonnummer. Wir senden Ihnen einen Code.
phone-unavailable = Die Anmeldung per SMS ist nicht verfügbar.
phone-label = Telefonnummer
# Eine Beispielnummer; eine im Format Ihrer Region verwenden.
phone-placeholder = +49 170 1234567
phone-send = Code senden
phone-code = Code
phone-submit = Anmelden
phone-another = Andere Nummer verwenden
phone-other-ways = Andere Anmeldemethoden

## Zweiter Schritt der Anmeldung

mfa-page-title = Zwei-Schritt-Anmeldung · { $site }
mfa-title = Bestätigen Sie, dass Sie es sind
mfa-description-pending = Ihr Konto verwendet die Zwei-Schritt-Anmeldung.
mfa-description-passkey = Verwenden Sie Ihren Passkey oder Sicherheitsschlüssel.
mfa-description-totp = Geben Sie den 6-stelligen Code aus Ihrer Authenticator-App ein.
mfa-description-recovery = Geben Sie einen der Wiederherstellungscodes ein, die Sie beim Aktivieren der Zwei-Schritt-Anmeldung gespeichert haben.
# Gefolgt von einem Link zur Anmeldeseite (mfa-expired-link).
mfa-expired = Dieser Anmeldeversuch ist abgelaufen.
mfa-expired-link = Erneut anmelden
mfa-passkey = Passkey oder Sicherheitsschlüssel verwenden
mfa-label-totp = Authentifizierungscode
mfa-label-recovery = Wiederherstellungscode
# Die Form eines Wiederherstellungscodes, in einem leeren Feld.
mfa-recovery-placeholder = xxxxx-xxxxx
mfa-submit = Weiter
mfa-use-totp = Authenticator-App verwenden
mfa-back = Zurück
mfa-use-recovery = Wiederherstellungscode verwenden

## Anmeldung mit dem Link aus einer E-Mail

magic-page-title = Anmelden · { $site }
magic-incomplete = Dieser Link ist unvollständig.
magic-failed-title = Wir konnten Sie nicht anmelden
# $reason ist ein Satz des Servers, der sagt, was schiefgelaufen ist; $minutes gibt an, wie lange
# Anmeldelinks gültig sind.
magic-failed-description = { $reason } Anmeldelinks sind einmalig und laufen nach { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } ab.
magic-title = Bei { $site } anmelden
magic-description = Sie haben einen Anmeldelink aus Ihrer E-Mail geöffnet.
magic-retry = Neuen Link senden
magic-continue = Weiter

## Registrierung

register-page-title = Konto erstellen · { $site }
register-title = Konto erstellen
register-description = Es dauert weniger als eine Minute.
register-username = Benutzername
register-username-hint = Melden Sie sich damit oder mit Ihrer E-Mail-Adresse an.
register-email = E-Mail-Adresse
register-password = Passwort
register-confirmation = Passwort bestätigen
register-submit = Konto erstellen
# Eine Frage, gefolgt vom Link, der sie beantwortet: "Sie haben bereits ein Konto? Anmelden".
register-have-account = Sie haben bereits ein Konto?
register-sign-in = Anmelden

## Registrierung, wenn die Adresse zuerst bestätigt werden muss

register-pending-title = Prüfen Sie Ihr Postfach
# $email ist die eingegebene Adresse. Sie wird hervorgehoben angezeigt.
register-pending-description = Wir haben einen Link an { $email } gesendet. Öffnen Sie ihn, um Ihre Adresse zu bestätigen, und melden Sie sich dann an.
register-pending-continue = Zur Anmeldung

## Passwort vergessen

forgot-page-title = Passwort zurücksetzen · { $site }
forgot-title = Passwort vergessen?
forgot-description = Geben Sie Ihre E-Mail-Adresse ein, und wir senden Ihnen einen Link zum Zurücksetzen.
forgot-email = E-Mail-Adresse
forgot-submit = Link zum Zurücksetzen senden
# Eine Frage, gefolgt vom Link, der sie beantwortet: "Doch wieder eingefallen? Anmelden".
forgot-remembered = Doch wieder eingefallen?
forgot-sign-in = Anmelden
forgot-sent-title = Prüfen Sie Ihr Postfach
# Dieselbe Antwort, ob es zu der Adresse ein Konto gibt oder nicht. $email wird hervorgehoben angezeigt;
# $minutes gibt an, wie lange der Link gültig ist.
forgot-sent-description = Falls ein Konto { $email } verwendet, haben wir dorthin einen Link gesendet, mit dem Sie ein neues Passwort wählen können. Der Link ist einmalig und läuft in { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } ab.
forgot-sent-back = Zurück zur Anmeldung

## Neues Passwort wählen (der Link aus der E-Mail zum Zurücksetzen)

reset-page-title = Neues Passwort wählen · { $site }
reset-title = Neues Passwort wählen
reset-description = Sie werden auf allen Geräten abgemeldet.
# Gefolgt von einem Link, um einen neuen anzufordern (reset-request-new).
reset-incomplete = Dieser Link ist unvollständig. Öffnen Sie den Link aus der E-Mail erneut.
# Wird nach einem Fehler angezeigt, gefolgt von einem Link (reset-request-new). $minutes gibt an,
# wie lange Links zum Zurücksetzen gültig sind.
reset-expiry = Links sind einmalig und laufen nach { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } ab.
reset-request-new = Neuen Link anfordern
reset-password = Neues Passwort
reset-confirmation = Neues Passwort bestätigen
reset-submit = Neues Passwort festlegen
reset-done = Ihr Passwort wurde geändert. Melden Sie sich mit dem neuen an.

## Adresse nach der Registrierung bestätigen (der Link aus der E-Mail)

verify-page-title = E-Mail-Adresse bestätigen · { $site }
verify-incomplete = Dieser Link ist unvollständig.
verify-verifying = Ihre E-Mail-Adresse wird bestätigt…
verify-done-title = E-Mail-Adresse bestätigt
verify-done-description = Vielen Dank! Ihre Adresse ist bestätigt.
verify-failed-title = Wir konnten Ihre E-Mail-Adresse nicht bestätigen
# $reason ist ein Satz des Servers, der sagt, was schiefgelaufen ist; $hours gibt an, wie lange
# Bestätigungslinks gültig sind.
verify-failed-description = { $reason } Links sind einmalig und laufen nach { $hours ->
        [one] { $hours } Stunde
       *[other] { $hours } Stunden
    } ab; Sie können in der App einen neuen anfordern.
verify-continue = Weiter zur App
verify-sign-in = Anmelden

## Eine neue Adresse bestätigen (der Link, der an sie gesendet wurde)

confirm-page-title = E-Mail-Adresse bestätigen · { $site }
confirm-incomplete = Dieser Link ist unvollständig.
confirm-confirming = Ihre neue Adresse wird bestätigt…
confirm-done-title = E-Mail-Adresse geändert
confirm-done-description = Verwenden Sie ab jetzt Ihre neue Adresse zur Anmeldung.
confirm-failed-title = Wir konnten Ihre E-Mail-Adresse nicht ändern
# $reason ist ein Satz des Servers, der sagt, was schiefgelaufen ist; $hours gibt an, wie lange
# Bestätigungslinks gültig sind.
confirm-failed-description = { $reason } Links sind einmalig und laufen nach { $hours ->
        [one] { $hours } Stunde
       *[other] { $hours } Stunden
    } ab; fordern Sie in Ihrem Profil einen neuen an.
confirm-profile = Zurück zu Ihrem Profil
confirm-sign-in = Anmelden

## Eine Adressänderung abbrechen (der Link, der an die alte Adresse gesendet wurde)

cancel-page-title = E-Mail-Änderung abbrechen · { $site }
cancel-incomplete = Dieser Link ist unvollständig.
cancel-title = Änderung der E-Mail-Adresse abbrechen?
cancel-description = Jemand hat angefordert, Ihr Konto auf eine andere Adresse umzustellen. Wenn Sie abbrechen, bleibt diese Adresse erhalten und alle Geräte werden abgemeldet.
cancel-submit = Änderung abbrechen
cancel-done-title = Änderung der E-Mail-Adresse abgebrochen
cancel-done-description = Ihr Konto behält diese Adresse, und alle Geräte wurden abgemeldet. Wenn Sie die Änderung nicht angefordert haben, kennt möglicherweise jemand Ihr Passwort: Wählen Sie ein neues.
cancel-new-password = Neues Passwort wählen
cancel-failed-title = Wir konnten die Änderung nicht abbrechen
cancel-sign-in = Anmelden
