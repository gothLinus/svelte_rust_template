## Der App-Rahmen: Layouts, Fehlerseiten und Meldungen, die zu keiner einzelnen Seite gehören.

# Der Titel im Browser-Tab für eine Seite innerhalb der App. $page ist der Name der Seite (Notizen,
# Profil, …), $site der Name der App.
app-page-title = { $page } · { $site }

# Der erste Link auf jeder Seite; Tastaturnutzer springen damit an der Navigation vorbei.
app-skip-to-content = Zum Inhalt springen

# Screenreader-Name des Logo-Links, der auf Smartphones zur Übersicht führt. $site ist der
# Name der App.
app-dashboard-link = Übersicht von { $site }

# Ein Hinweis, wenn der Server die Sitzung beendet, während die App geöffnet ist.
app-session-ended = Ihre Sitzung wurde beendet. Bitte melden Sie sich erneut an.

# Überschrift einer Fehlerseite ohne eigene Meldung.
app-error-fallback = Etwas ist schiefgelaufen

# Wird statt technischer Details angezeigt, wenn etwas Unerwartetes kaputtgegangen ist.
app-error-unexpected = Etwas ist schiefgelaufen. Bitte versuchen Sie es erneut.

# Nur in der Entwicklung: Die API läuft vermutlich nicht. $command ist ein Shell-Befehl und
# bleibt unübersetzt.
app-error-dev-hint = Läuft die API? Starten Sie alles mit { $command }.

app-error-retry = Erneut versuchen
app-error-back-to-start = Zurück zur Startseite
app-error-back-to-dashboard = Zurück zur Übersicht

# Die Meldung der Fehlerseite für eine unbekannte Adresse.
not-found-message = Diese Seite existiert nicht.
