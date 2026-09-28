use std::future::pending;

use tokio::{select, signal};

/// Resolves on Ctrl-C (SIGINT) or, on Unix, SIGTERM (what `docker stop` and Kubernetes send). The
/// server then stops accepting connections and lets in-flight requests finish.
pub async fn signal() {
    let interrupt = async {
        if signal::ctrl_c().await.is_err() {
            pending::<()>().await;
        }
    };

    #[cfg(unix)]
    let terminate = async {
        match signal::unix::signal(signal::unix::SignalKind::terminate()) {
            Ok(mut sigterm) => {
                sigterm.recv().await;
            }
            Err(_) => pending::<()>().await,
        }
    };

    #[cfg(not(unix))]
    let terminate = pending::<()>();

    select! {
        () = interrupt => tracing::info!("received SIGINT"),
        () = terminate => tracing::info!("received SIGTERM"),
    }
}
