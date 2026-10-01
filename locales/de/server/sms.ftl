## Textnachrichten (SMS und WhatsApp) mit einem Einmalcode. Sie sollen kurz bleiben.
## { $code } sind sechs Ziffern in zwei Gruppen ("123 456"), { $app } ist der Name der App und
## { $minutes } die Anzahl der Minuten, die der Code gültig ist.

sms-sign-in-code = { $code } ist Ihr Anmeldecode für { $app }. Er läuft in { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } ab. Geben Sie ihn niemals weiter.
sms-phone-verification-code = { $code } ist Ihr Bestätigungscode für { $app }. Er läuft in { $minutes ->
        [one] { $minutes } Minute
       *[other] { $minutes } Minuten
    } ab.
