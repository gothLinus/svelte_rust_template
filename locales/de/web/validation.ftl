## Clientseitige Prüfung von Formularfeldern. Sie spiegelt die Regeln und Formulierungen des Servers
## (`validation-*` im Katalog des Servers), damit ein Fehler gleich lautet, egal welche Seite ihn
## findet. Jede Meldung vervollständigt den Satz, den die Beschriftung des Felds beginnt.

validation-required = dieses Feld ist erforderlich
validation-invalid-email = geben Sie eine gültige E-Mail-Adresse ein

# { $min } ist die kleinste erlaubte Anzahl von Zeichen.
validation-too-short = { $min ->
        [one] muss mindestens { $min } Zeichen lang sein
       *[other] muss mindestens { $min } Zeichen lang sein
    }

# { $max } ist die größte erlaubte Anzahl von Zeichen.
validation-too-long = { $max ->
        [one] darf höchstens { $max } Zeichen lang sein
       *[other] darf höchstens { $max } Zeichen lang sein
    }

validation-password-only-spaces = darf nicht nur aus Leerzeichen bestehen
validation-username-invalid-characters = verwenden Sie Buchstaben, Ziffern, Punkte, Bindestriche und Unterstriche, beginnend mit einem Buchstaben oder einer Ziffer
validation-username-needs-letter = muss mindestens einen Buchstaben enthalten

# Das Beispiel ist eine Telefonnummer im internationalen Format.
validation-invalid-phone = geben Sie die Nummer mit Ländervorwahl ein, z. B. +49 170 1234567

validation-code = geben Sie den 6-stelligen Code ein
validation-control-characters = darf keine Steuerzeichen enthalten
validation-single-line = muss eine einzelne Zeile ohne Steuerzeichen sein
validation-passwords-differ = die Passwörter stimmen nicht überein
