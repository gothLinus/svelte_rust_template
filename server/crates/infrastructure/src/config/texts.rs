//! `TEXT_*` and `TWILIO_*`: whether text messages (SMS, WhatsApp) are sent and which countries
//! they may go to. Texting is off unless a transport is chosen; with Twilio off localhost,
//! `TEXT_ALLOWED_COUNTRIES` is mandatory (or `*` to opt out explicitly).

use domain::{
    secret::Secret,
    user::{CallingCode, PhoneNumber},
};

use super::reader::Reader;

#[derive(Debug, Clone)]
pub struct TextConfig {
    pub transport: TextTransport,
    /// Country calling codes (`+49`, `+1`) that numbers must start with to be texted
    /// (`TEXT_ALLOWED_COUNTRIES`). Empty allows every country (`*`, or unset on localhost). Keeping
    /// it to the countries the product serves stops texts being pumped to premium numbers
    /// elsewhere.
    pub allowed_countries: Vec<CallingCode>,
}

#[derive(Debug, Clone)]
pub enum TextTransport {
    Disabled,
    /// Logs messages instead of sending them (development only: the log holds the codes).
    Log,
    Twilio(TwilioConfig),
}

#[derive(Debug, Clone)]
pub struct TwilioConfig {
    pub account_sid: String,
    pub auth_token: Secret,
    pub sms_from: Option<PhoneNumber>,
    pub whatsapp_from: Option<PhoneNumber>,
}

pub(super) const TEXT_VARS: &[&str] = &[
    "TEXT_TRANSPORT",
    "TEXT_ALLOWED_COUNTRIES",
    "TWILIO_ACCOUNT_SID",
    "TWILIO_AUTH_TOKEN",
    "TWILIO_SMS_FROM",
    "TWILIO_WHATSAPP_FROM",
];

impl Reader<'_> {
    pub(super) fn texts(&mut self) -> TextConfig {
        let transport = match self.raw("TEXT_TRANSPORT").as_deref() {
            None | Some("none") => TextTransport::Disabled,
            Some("log") if !self.local => {
                self.problem(
                    "TEXT_TRANSPORT",
                    "`log` only works on localhost: codes would be logged instead of texted",
                );
                TextTransport::Disabled
            }
            Some("log") => TextTransport::Log,
            Some("twilio") => {
                let account_sid = self.required("TWILIO_ACCOUNT_SID", |raw| Ok(raw.to_owned()));
                let auth_token = self.required("TWILIO_AUTH_TOKEN", |raw| Ok(Secret::new(raw)));
                let sms_from = self.optional("TWILIO_SMS_FROM", phone);
                let whatsapp_from = self.optional("TWILIO_WHATSAPP_FROM", phone);
                if sms_from.is_none() && whatsapp_from.is_none() {
                    self.problem(
                        "TWILIO_SMS_FROM",
                        "set TWILIO_SMS_FROM, TWILIO_WHATSAPP_FROM or both",
                    );
                }
                match (account_sid, auth_token) {
                    (Some(account_sid), Some(auth_token)) => TextTransport::Twilio(TwilioConfig {
                        account_sid,
                        auth_token,
                        sms_from,
                        whatsapp_from,
                    }),
                    _ => TextTransport::Disabled,
                }
            }
            Some(_) => {
                self.problem("TEXT_TRANSPORT", "must be `none`, `log` or `twilio`");
                TextTransport::Disabled
            }
        };
        // `*` allows every country explicitly; unset means the same on localhost only.
        let every_country = self.raw("TEXT_ALLOWED_COUNTRIES").as_deref() == Some("*");
        if !self.local
            && !every_country
            && matches!(transport, TextTransport::Twilio(_))
            && self.raw("TEXT_ALLOWED_COUNTRIES").is_none()
        {
            self.problem(
                "TEXT_ALLOWED_COUNTRIES",
                "is required with TEXT_TRANSPORT=twilio unless APP_URL is localhost: list the \
                 calling codes you serve (+41,+49), or `*` for every country, which lets \
                 anyone pump texts to premium numbers at your expense",
            );
        }
        let allowed_countries = self.parse("TEXT_ALLOWED_COUNTRIES", Vec::new(), |raw| {
            if raw == "*" {
                return Ok(Vec::new());
            }
            raw.split(',')
                .map(str::trim)
                .filter(|code| !code.is_empty())
                .map(|code| {
                    CallingCode::parse(code)
                        .map_err(|_| format!("`{code}` is not a calling code such as +49"))
                })
                .collect()
        });
        TextConfig {
            transport,
            allowed_countries,
        }
    }
}

fn phone(raw: &str) -> Result<PhoneNumber, String> {
    PhoneNumber::parse(raw)
        .map_err(|_| "enter the number with its country code, e.g. +49 170 1234567".to_owned())
}
