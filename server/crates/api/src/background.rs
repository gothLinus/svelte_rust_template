//! Work a request starts but does not wait for.
//!
//! Endpoints that must not reveal whether an account exists (forgot password, emailed and texted
//! sign-in codes) answer before doing their work: issuing a token and queueing a message takes
//! measurably longer than finding no account, and the response time would otherwise tell the two
//! apart. Shutdown waits for what was started.

use std::{
    future::Future,
    sync::{Arc, Mutex, PoisonError},
};

use tokio::task::JoinSet;

/// Runs handed-off work: spawned in production, awaited on the spot in tests (so a test can check
/// the mail right after the response).
#[derive(Clone)]
pub struct Background(Mode);

#[derive(Clone)]
enum Mode {
    Spawn(Arc<Mutex<JoinSet<()>>>),
    Inline,
}

impl Background {
    pub fn spawning() -> Self {
        Self(Mode::Spawn(Arc::new(Mutex::new(JoinSet::new()))))
    }

    pub fn inline() -> Self {
        Self(Mode::Inline)
    }

    pub async fn run(&self, task: impl Future<Output = ()> + Send + 'static) {
        match &self.0 {
            Mode::Spawn(tasks) => {
                let mut tasks = tasks.lock().unwrap_or_else(PoisonError::into_inner);
                while tasks.try_join_next().is_some() {}
                tasks.spawn(task);
            }
            Mode::Inline => task.await,
        }
    }

    pub async fn drain(&self) {
        let Mode::Spawn(tasks) = &self.0 else {
            return;
        };
        let mut pending =
            std::mem::take(&mut *tasks.lock().unwrap_or_else(PoisonError::into_inner));
        while pending.join_next().await.is_some() {}
    }
}
