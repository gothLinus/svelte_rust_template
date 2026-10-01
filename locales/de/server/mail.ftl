## E-Mails. Reiner Text, ein Betreff und ein Inhalt je Mail, benannt
## `mail-<template>-subject` und `mail-<template>-body`.
##
## Platzhalter:
##   { $link }     eine vollständige URL; in einer eigenen Zeile lassen
##   { $code }     der Einmalcode, etwa "123 456"; in einer eigenen Zeile lassen
##   { $hours }    wie viele Stunden der Link gültig ist
##   { $minutes }  wie viele Minuten der Link oder Code gültig ist
##   { $email }    eine E-Mail-Adresse
##   { $what }     ein Satz aus den `notice-`-Nachrichten unten
## Ein Absatz ist eine Zeile; eine Leerzeile trennt Absätze.

mail-verify-email-subject = E-Mail-Adresse bestätigen
mail-verify-email-body =
    Willkommen! Bitte bestätigen Sie, dass dies Ihre E-Mail-Adresse ist, indem Sie diesen Link innerhalb von { $hours ->
        [one] { $hours } Stunde
       *[other] { $hours } Stunden
    } öffnen:

    { $link }

    Wenn Sie kein Konto erstellt haben, können Sie diese E-Mail ignorieren.

mail-password-reset-subject = Passwort zurücksetzen
mail-password-reset-body =
    Jemand, hoffentlich Sie, hat angefordert, das Passwort Ihres Kontos zurückzusetzen.

    Öffnen Sie diesen Link innerhalb von { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    }, um ein neues Passwort zu wählen:

    { $link }

    Wenn Sie dies nicht angefordert haben, ignorieren Sie diese E-Mail. Ihr Passwort bleibt unverändert.

# Wird gesendet, nachdem der erste Nachweis der Adresse ein Passwort entfernt hat, das die Inhaberin oder der Inhaber womöglich nicht selbst gewählt hat.
mail-choose-password-subject = Passwort festlegen
mail-choose-password-body =
    Ihre E-Mail-Adresse ist bestätigt. Das Konto wurde in einem anderen Browser registriert. Damit niemand sonst, der es eingerichtet haben könnte, Zugriff behält, wurde sein Passwort entfernt und andere Geräte wurden abgemeldet.

    Öffnen Sie diesen Link innerhalb von { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    }, um ein Passwort festzulegen:

    { $link }

    Sie können sich auch mit einem Code anmelden, der an diese Adresse gesendet wird, und jederzeit auf der Anmeldeseite einen neuen Link anfordern.

mail-password-changed-subject = Ihr Passwort wurde geändert
# { $link } startet das Zurücksetzen des Passworts.
mail-password-changed-body =
    Das Passwort Ihres Kontos wurde soeben geändert, und Ihre anderen Geräte wurden abgemeldet.

    Wenn Sie das nicht waren, setzen Sie Ihr Passwort sofort zurück:

    { $link }

# Der Code steht in einer eigenen Zeile, damit er leicht abzulesen und zu kopieren ist.
mail-sign-in-code-subject = Ihr Anmeldecode
mail-sign-in-code-body =
    Verwenden Sie diesen Code, um sich anzumelden:

    { $code }

    Oder öffnen Sie diesen Link auf dem Gerät, auf dem Sie sich anmelden möchten:

    { $link }

    Beide sind einmalig und { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } lang gültig. Wenn Sie nicht versucht haben, sich anzumelden, können Sie diese E-Mail ignorieren.

# Wird gesendet, wenn jemand versucht, eine Adresse zu registrieren, zu der es bereits ein Konto gibt.
# { $link } startet das Zurücksetzen des Passworts.
mail-already-registered-subject = Sie haben bereits ein Konto
mail-already-registered-body =
    Jemand, hoffentlich Sie, hat versucht, mit dieser E-Mail-Adresse ein Konto zu erstellen, aber es gibt bereits eines. Melden Sie sich stattdessen an, oder setzen Sie Ihr Passwort zurück, falls Sie es vergessen haben:

    { $link }

    Wenn Sie das nicht waren, können Sie diese E-Mail ignorieren.

# Der Code steht in einer eigenen Zeile, damit er leicht abzulesen und zu kopieren ist.
mail-reauthentication-code-subject = Bestätigen Sie, dass Sie es sind
mail-reauthentication-code-body =
    Geben Sie diesen Code ein, um eine Änderung an Ihrem Konto zu bestätigen:

    { $code }

    Er ist einmalig und { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } lang gültig. Wenn Sie nicht gerade versucht haben, Ihr Konto zu ändern, ist jemand anderes darin angemeldet: Setzen Sie Ihr Passwort zurück und melden Sie alle anderen Geräte ab.

# Wird an die alte Adresse gesendet. { $email } ist die neue (teilweise verborgen); { $link } bricht die Änderung ab.
mail-email-change-requested-subject = Ihre E-Mail-Adresse wird geändert
mail-email-change-requested-body =
    Jemand hat angefordert, die E-Mail-Adresse Ihres Kontos in { $email } zu ändern. Die Änderung erfolgt, sobald die neue Adresse bestätigt ist.

    Wenn Sie das nicht waren, brechen Sie die Änderung ab. Dabei werden auch alle Geräte abgemeldet:

    { $link }

    Setzen Sie anschließend Ihr Passwort zurück.

# { $link } startet das Zurücksetzen des Passworts.
mail-email-change-cancelled-subject = Die Änderung der E-Mail-Adresse wurde abgebrochen
mail-email-change-cancelled-body =
    Ihr Konto behält diese E-Mail-Adresse, und alle Geräte wurden abgemeldet.

    Möglicherweise kennt jemand anderes Ihr Passwort: Wählen Sie jetzt ein neues.

    { $link }

# Wird an die neue Adresse gesendet.
mail-confirm-email-change-subject = Neue E-Mail-Adresse bestätigen
mail-confirm-email-change-body =
    Öffnen Sie diesen Link innerhalb von { $hours ->
        [one] { $hours } Stunde
       *[other] { $hours } Stunden
    }, um diese Adresse zur E-Mail-Adresse Ihres Kontos zu machen:

    { $link }

    Wenn Sie dies nicht angefordert haben, ignorieren Sie diese E-Mail. Es ändert sich nichts.

# Wird bei einer Änderung der Sicherheitseinstellungen des Kontos gesendet. { $what } ist einer der
# `notice-`-Sätze; { $link } öffnet die Sicherheitseinstellungen.
mail-security-notice-subject = Eine Sicherheitseinstellung Ihres Kontos wurde geändert
mail-security-notice-body =
    { $what }

    Wenn Sie das waren, müssen Sie nichts tun. Falls nicht, melden Sie sich an, überprüfen Sie Ihre Sicherheitseinstellungen und melden Sie alle anderen Geräte ab:

    { $link }

## Die Sätze, mit denen ein Sicherheitshinweis beginnt (`{ $what }` oben).

# { $provider } ist der Name des Dienstes, etwa Google.
notice-identity-linked = Ihr { $provider }-Konto wurde mit Ihrem Konto verknüpft und kann jetzt zur Anmeldung verwendet werden.
# { $email } ist die neue Adresse, teilweise verborgen.
notice-email-changed = Die E-Mail-Adresse Ihres Kontos wurde in { $email } geändert.
# { $name } ist der Name, den die Person dem Passkey gegeben hat.
notice-passkey-added = Ihrem Konto wurde ein Passkey ({ $name }) hinzugefügt.
notice-passkey-removed = Ein Passkey wurde aus Ihrem Konto entfernt.
notice-totp-added = Ihrem Konto wurde eine Authenticator-App hinzugefügt.
notice-totp-removed = Die Authenticator-App wurde aus Ihrem Konto entfernt.
