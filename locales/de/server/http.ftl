## Antworten der HTTP-Schicht selbst: Anfragen, die nie einen Anwendungsfall erreichen.

## Der `title` eines Problem-Dokuments: der Name seines HTTP-Status.

http-status-400 = Ungültige Anfrage
http-status-401 = Nicht autorisiert
http-status-403 = Zugriff verweigert
http-status-404 = Nicht gefunden
http-status-405 = Methode nicht erlaubt
http-status-409 = Konflikt
http-status-411 = Länge erforderlich
http-status-412 = Vorbedingung fehlgeschlagen
http-status-413 = Anfrage zu groß
http-status-415 = Nicht unterstützter Medientyp
http-status-422 = Nicht verarbeitbare Anfrage
http-status-429 = Zu viele Anfragen
http-status-500 = Interner Serverfehler
http-status-502 = Fehlerhaftes Gateway
http-status-503 = Dienst nicht verfügbar
# Jeder andere Status.
http-status-other = Fehler

## Das `detail` eines Problem-Dokuments.

http-method-not-allowed = diese Methode ist nicht erlaubt
# Zu viele Anfragen oder Versuche von einer Adresse oder gegen ein Konto.
http-rate-limited = zu viele Versuche, versuchen Sie es später erneut
http-timeout = die Anfrage hat zu lange gedauert
# Für Entwickler von API-Clients. { $type } ist ein Medientyp, nicht übersetzen.
http-unsupported-media-type = erwartet wird eine Anfrage mit `Content-Type: { $type }`
http-invalid-body = der Inhalt der Anfrage ist keine gültige Nachricht
http-invalid-request = die Anfrage konnte nicht gelesen werden
http-payload-too-large = der Inhalt der Anfrage ist zu groß
# length_required: ein Upload ohne Content-Length-Header, für Entwickler von API-Clients.
http-length-required = Uploads benötigen einen Content-Length-Header
http-invalid-query = der Query-String ist ungültig
# Für Entwickler von API-Clients. X-Requested-With ist ein Header-Name, nicht übersetzen.
http-csrf-header-required = zustandsändernde Anfragen benötigen einen X-Requested-With-Header
http-csrf-cross-origin = Cross-Origin-Anfrage abgelehnt
