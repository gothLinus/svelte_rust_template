## Bausteine der Anmelde- und Registrierungsseiten: gemeinsam genutzte Eingabefelder und Schaltflächen.

# Beispiel eines Einmalcodes im Eingabefeld (sechs Ziffern, ein Leerzeichen in der Mitte).
signin-code-placeholder = 123 456

# Screenreader-Name der Schaltfläche, die das Passwort einblendet; er bleibt beim Drücken gleich.
signin-show-password = Passwort anzeigen

# Das Wort zwischen zwei Anmeldemethoden, auf einer Trennlinie.
signin-or = oder

# Beschriftung der Auswahl, ob ein Code per SMS oder per WhatsApp gesendet wird.
signin-channel-label = Code senden per
signin-channel-sms = SMS
signin-channel-whatsapp = WhatsApp
signin-channel-text = Textnachricht

## Warum eine Anmeldung über einen externen Anbieter mit einem Fehler zurückkam. Die Id endet mit
## dem Code des Servers, `_` als `-` geschrieben.

signin-oauth-error-oauth-cancelled = Die Anmeldung über den Anbieter wurde abgebrochen.
signin-oauth-error-oauth-state-invalid = Dieser Anmeldeversuch ist abgelaufen oder wurde in einem anderen Browser gestartet. Bitte versuchen Sie es erneut.
signin-oauth-error-email-in-use = Es gibt bereits ein Konto mit dieser E-Mail-Adresse. Melden Sie sich dort an und verknüpfen Sie dann den Anbieter unter Einstellungen → Sicherheit.
signin-oauth-error-email-required = Der Anbieter hat uns keine E-Mail-Adresse übermittelt.
signin-oauth-error-provider-unavailable = Der Anbieter ist nicht erreichbar. Bitte versuchen Sie es erneut.
signin-oauth-error-identity-taken = Dieses Konto ist bereits mit einem anderen Benutzer verknüpft.
signin-oauth-error-provider-linked = Es ist bereits ein anderes Konto dieses Anbieters verknüpft. Heben Sie zuerst diese Verknüpfung auf.
signin-oauth-error-account-disabled = Dieses Konto wurde deaktiviert.
signin-oauth-error-email-not-verified = Bestätigen Sie Ihre E-Mail-Adresse, um sich anzumelden. Wir haben Ihnen einen Link gesendet.
signin-oauth-error-email-unverified = Der Anbieter hat diese E-Mail-Adresse nicht bestätigt. Melden Sie sich auf andere Weise an oder bestätigen Sie zuerst Ihre Adresse.
signin-oauth-error-reauth-required = Bestätigen Sie, dass Sie es sind, und verknüpfen Sie das Konto dann erneut.
signin-oauth-error-rate-limited = Zu viele Versuche. Bitte warten Sie einen Moment und versuchen Sie es erneut.
signin-oauth-error-not-found = Dieser Anmeldeanbieter ist nicht verfügbar.
signin-oauth-error-unauthenticated = Melden Sie sich zuerst an, um ein Konto zu verknüpfen.

# Jeder andere Code.
signin-oauth-error-failed = Die Anmeldung über den Anbieter ist fehlgeschlagen. Bitte versuchen Sie es erneut.

## Sitzung

# Banner, wenn die App den Server nicht fragen konnte, ob jemand angemeldet ist.
session-error-title = Wir konnten nicht prüfen, ob Sie angemeldet sind
session-error-retry = Erneut versuchen

## Geräte

# Die Bezeichnung eines angemeldeten Geräts. { $browser } und { $system } sind Namen wie Firefox und macOS.
session-device = { $browser } auf { $system }
session-device-unknown = Unbekanntes Gerät

## Hinweis zur Bestätigung der E-Mail-Adresse

email-verification-title = Bitte bestätigen Sie Ihre E-Mail-Adresse
# { $email } ist die Adresse, an die ein Link gesendet wurde.
email-verification-body = Wir haben einen Link an { $email } gesendet. Öffnen Sie ihn, um zu bestätigen, dass die Adresse Ihnen gehört.
email-verification-resend = Link erneut senden
email-verification-resent = Wir haben einen neuen Link an { $email } gesendet.
