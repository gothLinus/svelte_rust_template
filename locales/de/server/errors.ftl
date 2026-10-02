## Das `detail` eines Problem-Dokuments: was schiefgelaufen ist, für die Person, die die App nutzt.
## Eine Nachricht je Art von Anwendungsfehler, benannt `error-<code>`. Der maschinenlesbare
## `code` daneben ändert sich nie mit der Sprache.

error-validation-failed = die Anfrage ist ungültig
error-unauthenticated = Anmeldung erforderlich
# Bewusst vage: Es wird nicht verraten, ob das Konto oder das Passwort falsch war.
error-invalid-credentials = E-Mail-Adresse, Benutzername oder Passwort ist ungültig
error-email-not-verified = bestätigen Sie Ihre E-Mail-Adresse, um sich anzumelden
error-account-disabled = dieses Konto wurde deaktiviert
error-forbidden = Sie haben keine Berechtigung für diese Aktion
error-reauth-required = bestätigen Sie, dass Sie es sind, um fortzufahren
# Auch die Antwort für Dinge, die die Person nicht sehen darf, damit sich damit nicht nach ihnen suchen lässt.
error-not-found = nicht gefunden
error-stale = dies wurde zwischenzeitlich von jemand anderem geändert, laden Sie es neu und versuchen Sie es noch einmal
error-invalid-token = dieser Link ist ungültig oder abgelaufen
error-invalid-passkey = dieser Passkey konnte nicht überprüft werden
error-provider-unavailable = der Anmeldeanbieter ist nicht erreichbar, versuchen Sie es erneut
error-busy = der Server ist ausgelastet, versuchen Sie es gleich noch einmal
# Wird bei jedem unerwarteten Fehler angezeigt; die Ursache steht nur im Server-Log.
error-internal = ein unerwarteter Fehler ist aufgetreten

## Konflikte: Die Anfrage ist in Ordnung, widerspricht aber dem aktuellen Zustand.
## Jeder hat seinen eigenen stabilen `code` (im Kommentar), auf den Clients reagieren können.

# email_taken
conflict-email-taken = es gibt bereits ein Konto mit dieser E-Mail-Adresse
# email_taken: Die Adresse einer ausstehenden E-Mail-Änderung wurde inzwischen von jemand anderem registriert.
conflict-old-address-taken = die alte Adresse wird inzwischen von einem anderen Konto verwendet
# email_unverified
conflict-email-unverified = bestätigen Sie zuerst Ihre E-Mail-Adresse
# already_verified
conflict-already-verified = diese E-Mail-Adresse ist bereits bestätigt
# cannot_disable_self
conflict-cannot-disable-self = Sie können Ihr eigenes Konto nicht deaktivieren
# last_admin
conflict-last-admin = mindestens ein aktives Konto muss Benutzer verwalten können
# channel_unavailable. { $channel } ist der Weg, auf dem der Code gesendet würde: email, sms oder whatsapp.
conflict-channel-unavailable = das Senden von Codes per { $channel ->
        [sms] SMS
        [whatsapp] WhatsApp
       *[email] E-Mail
    } ist nicht eingerichtet
# totp_enabled
conflict-totp-enabled = es ist bereits eine Authenticator-App eingerichtet; entfernen Sie sie zuerst
# totp_not_started
conflict-totp-not-started = beginnen Sie zuerst mit der Einrichtung der App
# mfa_disabled
conflict-mfa-disabled = aktivieren Sie zuerst die Zwei-Schritt-Anmeldung
# mfa_expired
conflict-mfa-expired = dieser Anmeldeversuch ist abgelaufen, melden Sie sich erneut an
# passkey_exists
conflict-passkey-exists = dieser Passkey ist bereits registriert

## Anmeldung über einen externen Anbieter (Social Login). { $provider } ist sein Name, etwa Google.

# oauth_cancelled
conflict-oauth-cancelled = die Anmeldung über den Anbieter wurde abgebrochen
# email_required
conflict-oauth-email-required = der Anbieter hat keine E-Mail-Adresse übermittelt
# email_unverified
conflict-oauth-email-unverified = { $provider } hat diese E-Mail-Adresse nicht bestätigt; registrieren Sie sich zuerst mit dieser Adresse und verknüpfen Sie dann { $provider } in Ihren Sicherheitseinstellungen
# email_in_use
conflict-oauth-email-in-use = es gibt bereits ein Konto mit dieser E-Mail-Adresse; melden Sie sich dort an und verknüpfen Sie { $provider } in Ihren Sicherheitseinstellungen
# username_taken
conflict-oauth-username-taken = der erzeugte Benutzername wurde gerade vergeben; bitte versuchen Sie es erneut
# username_taken
conflict-oauth-username-unavailable = es wurde kein freier Benutzername gefunden; bitte versuchen Sie es erneut
# provider_linked
conflict-provider-linked = es ist bereits ein anderes Konto dieses Anbieters verknüpft; heben Sie zuerst diese Verknüpfung auf
# oauth_state_invalid
conflict-oauth-state-invalid = dieser Anmeldeversuch ist abgelaufen oder wurde in einem anderen Browser gestartet, versuchen Sie es erneut
# identity_taken
conflict-identity-taken = dieses Konto ist bereits mit einem anderen Benutzer verknüpft
