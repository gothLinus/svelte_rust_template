use infrastructure::config::LogFormat;
use tracing_subscriber::{EnvFilter, fmt};

pub const DEFAULT_FILTER: &str =
    "info,api=debug,application=debug,infrastructure=info,domain=info,tower_http=info,sqlx=warn";

pub type TelemetryError = Box<dyn std::error::Error + Send + Sync>;

pub fn init(format: LogFormat) -> Result<(), TelemetryError> {
    let filter =
        EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(DEFAULT_FILTER));
    let subscriber = fmt().with_env_filter(filter).with_writer(std::io::stderr);

    match format {
        LogFormat::Text => subscriber.try_init(),
        LogFormat::Json => subscriber.json().flatten_event(true).try_init(),
    }
}
