## Text messages (SMS and WhatsApp) with a one-time code. They should stay short.
## { $code } is six digits in two groups ("123 456"), { $app } the name of the app and
## { $minutes } the number of minutes the code works.

sms-sign-in-code = { $code } is your { $app } sign-in code. It expires in { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }. Never share it.
sms-phone-verification-code = { $code } is your { $app } verification code. It expires in { $minutes ->
        [one] { $minutes } minute
       *[other] { $minutes } minutes
    }.
