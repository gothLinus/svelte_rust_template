## Meldungen neben einem Formularfeld, das falsch ausgefüllt wurde. Sie vervollständigen die
## Beschriftung des Felds und beginnen daher klein: "Benutzername: muss mindestens 3 Zeichen lang sein".
## Jede hat einen stabilen `code` (im Kommentar), auf den Clients reagieren können.

# required
validation-required = dieses Feld ist erforderlich

## Anmeldekennungen

# invalid_email
validation-invalid-email = geben Sie eine gültige E-Mail-Adresse ein
# invalid_phone. Das Beispiel ist eine Nummer mit Ländervorwahl.
validation-invalid-phone = geben Sie die Nummer mit Ländervorwahl ein, z. B. +49 170 1234567
# invalid_calling_code. { $code } ist die Eingabe.
validation-invalid-calling-code = `{ $code }` ist keine Ländervorwahl wie +49
# phone_taken
validation-phone-taken = diese Nummer wird von einem anderen Konto verwendet
# phone_country_unsupported
validation-phone-country-unsupported = Nummern aus diesem Land werden nicht unterstützt
# unchanged
validation-email-unchanged = das ist Ihre aktuelle Adresse

## Sprachen.

# unsupported_locale
validation-locale-unsupported = keine Sprache, in der diese App verfügbar ist

## Benutzernamen. { $min } und { $max } sind Anzahlen von Zeichen.

# too_short
validation-username-too-short = muss mindestens { $min } Zeichen lang sein
# too_long
validation-username-too-long = darf höchstens { $max } Zeichen lang sein
# invalid_username
validation-username-invalid-characters = verwenden Sie Buchstaben, Ziffern, Punkte, Bindestriche und Unterstriche, beginnend mit einem Buchstaben oder einer Ziffer
# invalid_username
validation-username-needs-letter = muss mindestens einen Buchstaben enthalten
# username_taken
validation-username-taken = dieser Benutzername ist bereits vergeben

## Passwörter. { $min } und { $max } sind Anzahlen von Zeichen.

# too_short
validation-password-too-short = muss mindestens { $min } Zeichen lang sein
# too_long
validation-password-too-long = darf höchstens { $max } Zeichen lang sein
# too_weak
validation-password-only-spaces = darf nicht nur aus Leerzeichen bestehen
# too_common
validation-password-too-common = dieses Passwort gehört zu den häufigsten; wählen Sie ein anderes
# incorrect_password
validation-incorrect-password = das Passwort ist falsch
# invalid_credential
validation-invalid-credential = das ist nicht korrekt

## An die Person gesendete Codes

# invalid_code: ein Code aus einer E-Mail, einer SMS oder einer Authenticator-App oder ein Wiederherstellungscode.
validation-invalid-code = dieser Code ist ungültig oder abgelaufen

## Passkeys. { $max } ist eine Anzahl von Zeichen.

# too_long
validation-passkey-name-too-long = darf höchstens { $max } Zeichen lang sein
# invalid_characters
validation-passkey-name-control-characters = darf keine Steuerzeichen enthalten

## Listen

# out_of_range. Seitengrößen: { $min } und { $max } sind Anzahlen von Einträgen.
validation-page-size-out-of-range = muss zwischen { $min } und { $max } liegen
# invalid_cursor
validation-invalid-cursor = kein gültiger Seiten-Cursor
# too_long. { $max } ist eine Anzahl von Zeichen.
validation-search-too-long = darf höchstens { $max } Zeichen lang sein
# too_short. { $min } ist eine Anzahl von Zeichen.
validation-search-too-short = muss mindestens { $min } Zeichen lang sein

## Werte der API, die keine Person eintippt; sie tauchen auf, wenn ein Client Unsinn sendet.

# invalid
validation-invalid-uuid = muss eine UUID sein
# invalid
validation-not-allowed-value = ist keiner der erlaubten Werte
# invalid_id
validation-invalid-id = keine gültige ID
# invalid_role
validation-invalid-role = kein gültiger Rollenname
# unknown_permission. { $permission } ist der gesendete Name.
validation-unknown-permission = unbekannte Berechtigung `{ $permission }`
