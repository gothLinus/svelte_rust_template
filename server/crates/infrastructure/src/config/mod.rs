//! Typed configuration, read from environment variables (and `.env` in development).
//!
//! Everything is validated at startup: a bad value stops the process with a list of every
//! problem, instead of failing on the first request that needs it. `.env.example` at the
//! repository root documents each variable.
//!
//! Development conveniences fail closed: when `APP_URL` is not localhost, the mail transport must
//! be chosen explicitly, mail and texts cannot go to the log, and the published development
//! `SECRET_KEY` is refused.
//!
//! [`Config`] is the whole configuration; each section lives in its own submodule that reads its
//! variables through the private `Reader`, which applies defaults and collects problems. To add a
//! setting: add the field, read it in its section's `Reader` method with `parse`, `required`,
//! `bool`, `number` or `duration` (durations are always bounded), and document the variable in
//! `.env.example`. A misspelled variable in a family that has a known prefix (`OAUTH_*`,
//! `RATE_LIMIT_*`, `TEXT_*`, `TWILIO_*`) becomes a startup warning; a new variable in
//! those families must be added to the matching list of known names.

use std::fmt::{self, Display, Formatter};

use thiserror::Error;

pub use auth::{AuthConfig, SecretKey};
pub use database::DatabaseConfig;
pub use http::{HttpConfig, PublicOrigin, RateLimitStore};
pub use mail::{MailConfig, MailTransport};
pub use oauth::{ClientSecret, OAuthConfig, OAuthProviderConfig};
pub use reader::parse_duration;
pub use storage::{DEFAULT_QUOTA_PER_USER, StorageConfig};
pub use telemetry::{OtlpConfig, TelemetryConfig};
pub use texts::{TextConfig, TextTransport, TwilioConfig};

mod auth;
mod database;
mod http;
mod mail;
mod oauth;
mod reader;
mod storage;
mod telemetry;
mod texts;

#[derive(Debug)]
pub struct Config {
    pub http: HttpConfig,
    pub database: DatabaseConfig,
    pub mail: MailConfig,
    pub auth: AuthConfig,
    pub texts: TextConfig,
    pub oauth: OAuthConfig,
    pub telemetry: TelemetryConfig,
    pub storage: StorageConfig,
    pub log_format: LogFormat,
    /// Things worth a warning at startup that do not stop it, such as a misspelled `OAUTH_*` or
    /// `TWILIO_*` variable that nothing reads.
    pub warnings: Vec<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LogFormat {
    Text,
    Json,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ConfigProblem {
    pub variable: &'static str,
    pub message: String,
}

#[derive(Debug, Error)]
pub struct ConfigError(pub Vec<ConfigProblem>);

impl Display for ConfigError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        write!(f, "invalid configuration:")?;
        for problem in &self.0 {
            write!(f, "\n  {}: {}", problem.variable, problem.message)?;
        }
        Ok(())
    }
}

#[derive(Debug, Error)]
#[error("failed to load .env")]
pub struct DotenvError(#[source] dotenvy::Error);

/// Loads `.env` from the working directory or the closest parent that has one. A missing file is
/// fine: production sets real environment variables. Call it before [`Config::from_env`];
/// variables already in the environment win over the file.
///
/// # Errors
///
/// Fails if a `.env` file is found but cannot be read or parsed.
pub fn load_dotenv() -> Result<(), DotenvError> {
    match dotenvy::dotenv() {
        Ok(_) => Ok(()),
        Err(err) if err.not_found() => Ok(()),
        Err(err) => Err(DotenvError(err)),
    }
}

impl Config {
    /// Reads and validates the configuration from the process environment.
    ///
    /// Also appends warnings for unknown `OAUTH_*`, `RATE_LIMIT_*`, `TEXT_*` and `TWILIO_*`
    /// variables (see [`unknown_variables`]).
    ///
    /// # Errors
    ///
    /// Returns a [`ConfigError`] listing every missing, malformed or unsafe setting at once, so a
    /// deployment can be fixed in one pass. Unsafe means production-unsafe values off localhost
    /// (see the module docs), not only unparsable ones.
    pub fn from_env() -> Result<Self, ConfigError> {
        let mut config = Self::from_lookup(|name| std::env::var(name).ok())?;
        config
            .warnings
            .extend(unknown_variables(std::env::vars().map(|(name, _)| name)));
        Ok(config)
    }

    /// Reads the configuration through `lookup`, which returns a variable's value if it is set.
    /// Tests pass a map instead of touching the process environment. Fails like
    /// [`Config::from_env`] but does not check for unknown variables.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut env = reader::Reader {
            lookup: &lookup,
            problems: Vec::new(),
            local: true,
            warnings: Vec::new(),
        };

        // First: whether `APP_URL` is localhost decides what the other sections accept.
        let http = env.http();
        let database = env.database();
        let mail = env.mail();
        let auth = env.auth();
        let texts = env.texts();
        let oauth = env.oauth();
        // Without an `AuthConfig` the configuration is refused anyway.
        let telemetry = env.telemetry(auth.as_ref().map_or("", |auth| auth.app_name.as_str()));
        let storage = env.storage();
        let log_format = env.parse("LOG_FORMAT", LogFormat::Text, |raw| match raw {
            "text" => Ok(LogFormat::Text),
            "json" => Ok(LogFormat::Json),
            _ => Err("must be `text` or `json`".to_owned()),
        });

        match (http, database, mail, auth, storage) {
            (Some(http), Some(database), Some(mail), Some(auth), Some(storage))
                if env.problems.is_empty() =>
            {
                Ok(Self {
                    http,
                    database,
                    mail,
                    auth,
                    texts,
                    oauth,
                    telemetry,
                    storage,
                    log_format,
                    warnings: env.warnings,
                })
            }
            _ => Err(ConfigError(env.problems)),
        }
    }
}

/// Warnings for `OAUTH_*`, `TWILIO_*` and `RATE_LIMIT_*` variables nothing reads:
/// most likely typos (`OAUTH_GOOGLE_CLIENTID`) that would otherwise silently leave a feature off
/// or a limit at its default.
pub fn unknown_variables(names: impl IntoIterator<Item = String>) -> Vec<String> {
    let mut unknown: Vec<String> = names
        .into_iter()
        .filter(|name| {
            (name.starts_with("OAUTH_") && !oauth::is_provider_var(name))
                || (name.starts_with("RATE_LIMIT_") && !http::RATE_VARS.contains(&name.as_str()))
                || ((name.starts_with("TWILIO_") || name.starts_with("TEXT_"))
                    && !texts::TEXT_VARS.contains(&name.as_str()))
                || (name.starts_with("STORAGE_") && !storage::STORAGE_VARS.contains(&name.as_str()))
        })
        .collect();
    unknown.sort();
    unknown
        .into_iter()
        .map(|name| format!("{name} is set but not a known variable; is it misspelled?"))
        .collect()
}
