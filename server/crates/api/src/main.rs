//! The server binary.
//!
//! ```text
//! api                       run the server (same as `api serve`)
//! api migrate               apply pending migrations and exit
//! api create-admin <email>  give an existing account the admin role
//! api healthcheck           exit 0 if the local server answers /health/live
//! ```

use std::{env, net::SocketAddr, process::ExitCode};

use api::{
    app::{self, StartupError},
    telemetry,
};
use domain::error::ErrorChain;
use infrastructure::config::{self, Config};
use tokio::runtime;

/// mimalloc instead of the system allocator. glibc's malloc contends on its arenas under many
/// threads doing small allocations, which is what a Tokio server does, and it tends to hold on to
/// Argon2's 19 MiB per-hash buffers instead of returning them to the OS.
#[global_allocator]
static GLOBAL: mimalloc::MiMalloc = mimalloc::MiMalloc;

const USAGE: &str = "usage: api [serve | migrate | create-admin <email> | healthcheck]";

enum Command {
    Serve,
    Migrate,
    CreateAdmin(String),
    Healthcheck,
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let command = match args
        .iter()
        .map(String::as_str)
        .collect::<Vec<_>>()
        .as_slice()
    {
        [] | ["serve"] => Command::Serve,
        ["migrate"] => Command::Migrate,
        ["create-admin", email] => Command::CreateAdmin((*email).to_owned()),
        ["healthcheck"] => Command::Healthcheck,
        _ => {
            eprintln!("{USAGE}");
            return ExitCode::from(2);
        }
    };

    match run(command) {
        Ok(()) => ExitCode::SUCCESS,
        Err(err) => {
            eprintln!("api: {}", ErrorChain(&err));
            ExitCode::FAILURE
        }
    }
}

fn run(command: Command) -> Result<(), StartupError> {
    config::load_dotenv()?;

    if let Command::Healthcheck = command {
        // Needs nothing but the port, so it works even if other variables are missing.
        let addr = env::var("BIND_ADDRESS")
            .ok()
            .and_then(|raw| raw.parse().ok())
            .unwrap_or(SocketAddr::from(([127, 0, 0, 1], 3000)));
        return runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(StartupError::Runtime)?
            .block_on(app::healthcheck(addr));
    }

    let config = Config::from_env()?;
    // Only the server is worth tracing; the one-off commands would wait for a flush at exit.
    let otlp = match command {
        Command::Serve => config.telemetry.otlp.as_ref(),
        _ => None,
    };
    let telemetry = telemetry::init(config.log_format, otlp).map_err(StartupError::Telemetry)?;
    for warning in &config.warnings {
        tracing::warn!("{warning}");
    }
    let runtime = runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .map_err(StartupError::Runtime)?;

    let result = match command {
        Command::Serve => runtime.block_on(app::serve(config)),
        Command::Migrate => runtime.block_on(app::migrate(&config)),
        Command::CreateAdmin(email) => runtime.block_on(app::create_admin(&config, &email)),
        Command::Healthcheck => Ok(()),
    };
    // Off the runtime: the exporters' blocking HTTP clients must not be dropped on it.
    drop(runtime);
    telemetry.shutdown();
    result
}
