use std::sync::Arc;

use domain::{
    error::ErrorChain,
    mail::{Mail, MailError, MailFuture, Mailer},
};
use thiserror::Error;
use tokio::{sync::mpsc, task::JoinHandle};

pub const QUEUE_CAPACITY: usize = 1024;

#[derive(Debug, Error)]
#[error("the mail queue is full or shut down")]
pub struct QueueFull;

/// Queues mail for a background task that delivers it through another [`Mailer`].
///
/// Requests only wait for the enqueue, not for the SMTP round trip: a slow relay never holds up a
/// response, and a request that sends mail (a reset link for an existing account) takes as long as
/// one that does not (an unknown address).
#[derive(Clone)]
pub struct MailQueue {
    sender: mpsc::Sender<Mail>,
}

/// The delivery task. Once every [`MailQueue`] handle is dropped it delivers what is left and
/// finishes; await it during shutdown so queued mail is not lost.
pub struct MailWorker(JoinHandle<()>);

impl MailWorker {
    pub async fn finish(self) {
        if let Err(err) = self.0.await {
            tracing::error!(error = %err, "mail worker panicked");
        }
    }
}

impl MailQueue {
    pub fn start(transport: Arc<dyn Mailer>) -> (Self, MailWorker) {
        let (sender, mut receiver) = mpsc::channel::<Mail>(QUEUE_CAPACITY);
        let worker = tokio::spawn(async move {
            while let Some(mail) = receiver.recv().await {
                let to = mail.to.masked();
                if let Err(err) = transport.send(mail).await {
                    tracing::error!(to, error = %ErrorChain(&err), "failed to deliver mail");
                }
            }
        });
        (Self { sender }, MailWorker(worker))
    }
}

impl Mailer for MailQueue {
    fn send(&self, mail: Mail) -> MailFuture<'_> {
        let queued = self
            .sender
            .try_send(mail)
            .map_err(|_| MailError::new(QueueFull));
        Box::pin(async move { queued })
    }
}
