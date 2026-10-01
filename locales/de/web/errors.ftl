## Im Browser festgestellte Fehler. Fehler, mit denen der Server antwortet, formuliert der Server
## selbst, in der Sprache, die die App in `Accept-Language` sendet.

error-network = Der Server ist nicht erreichbar. Prüfen Sie Ihre Verbindung und versuchen Sie es erneut.
error-timeout = Der Server hat zu lange nicht geantwortet. Bitte versuchen Sie es erneut.
error-aborted = Die Anfrage wurde abgebrochen.
error-invalid-response = Der Server hat eine Antwort gesendet, die wir nicht lesen konnten.
error-unknown = Etwas ist schiefgelaufen. Bitte versuchen Sie es erneut.

# Eine fehlgeschlagene Antwort, die kein Problem-Dokument ist und keinen Statustext hat (z. B. von einem Proxy).
# { $status } ist der HTTP-Statuscode.
error-request-failed = Die Anfrage ist mit Status { $status } fehlgeschlagen.

# Ersetzt die Formulierung des Servers für zwei Codes, bei denen sie nicht sagt, was als Nächstes zu tun ist.
error-email-unverified = Bestätigen Sie zuerst Ihre E-Mail-Adresse: Öffnen Sie den Link, den wir Ihnen gesendet haben, oder fordern Sie über den Hinweis oben auf der Seite einen neuen an.
error-reauth-required = Bestätigen Sie, dass Sie es sind, um diese Änderung vorzunehmen.

# Wird auf der Fehlerseite angezeigt, wenn jemand eine Seite ohne die nötige Berechtigung öffnet.
error-forbidden = Sie haben keine Berechtigung, diese Seite anzusehen.

# Ein Feld, für das das Formular keine Eingabe hat, wurde vom Server abgelehnt.
# { $field } ist der Name des Felds, z. B. "Neues Passwort"; { $message } vervollständigt den Satz.
form-field-problem = { $field }: { $message }.
