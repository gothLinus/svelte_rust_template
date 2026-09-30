//! Durable delivery of mail and text messages through the `outbox` table.
//!
//! The application hands messages to [`OutboxMailer`] and [`OutboxTexts`], the [`Mailer`] and
//! [`TextSender`] it is given in production. They only insert a row, so a request never waits for
//! an SMTP relay or Twilio. An [`OutboxWorker`] claims due rows (`for update skip locked`, so
//! several instances share the work), hands them to the real transport, deletes what was accepted
//! and retries what failed with exponential backoff. A message is only gone once delivered or after
//! [`MAX_ATTEMPTS`] failures, which are logged with the transport's error. Delivery is
//! at-least-once: if a worker dies after the transport accepted a message but before the row is
//! deleted, the message is sent again once its lease runs out.
//!
//! To send a new kind of message through the outbox, add a payload type and a `kind` constant
//! here, an enqueue path like `OutboxMailer::send`, and a branch in the worker's delivery. The rest
//! of the application only sees the [`Mailer`] and [`TextSender`] ports.
//!
//! Messages are written right after the change that triggers them is committed, not in the same
//! transaction: a crash in the milliseconds between the two loses that one message. Every mail the
//! app sends can be asked for again (a new link, a new code), so that window is accepted in return
//! for keeping the ports independent of storage.
//!
//! Pending messages hold working links and codes, so their payload is sealed with the [`Crypto`]
//! port (AES-256-GCM under `SECRET_KEY`, bound to the row's id and kind): a dump, a replica or the
//! WAL shows only ciphertext. A message also expires with what it carries ([`Mail::valid_for`],
//! [`TextMessage::valid_for`]). Rows are linked to the recipient's account when the address has
//! one, so deleting the account deletes its undelivered mail.

use std::{sync::Arc, time::Duration};

use domain::{
    error::{ErrorChain, StorageError},
    mail::{Mail, MailError, MailFuture, Mailer},
    one_time_code::CodeChannel,
    security::Crypto,
    text::{TextError, TextFuture, TextMessage, TextSender},
    user::{Email, PhoneNumber},
};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use tokio::{
    sync::{Notify, watch},
    task::JoinHandle,
    time::timeout,
};
use uuid::Uuid;

use crate::db::errors::{corrupt, db_error};

/// Deliveries tried per message before it is dropped: with the backoff below (10 s, doubling each
/// time), the retries span about six hours.
pub const MAX_ATTEMPTS: i32 = 12;
/// The longest a message is kept, and how long one without a [`Mail::valid_for`] (a notice) lives,
/// whatever the retries.
const MAX_RETENTION: time::Duration = time::Duration::days(2);
const LEASE_SECONDS: f64 = 60.0;
const BATCH: i64 = 32;
/// How often a worker looks for due messages when nothing wakes it: retries, and messages other
/// instances enqueued.
const POLL: Duration = Duration::from_secs(5);
const DELIVERY_TIMEOUT: Duration = Duration::from_secs(30);

#[derive(Serialize, Deserialize)]
struct MailPayload {
    to: String,
    template: String,
    subject: String,
    body: String,
}

const MAIL: &str = "mail";
const TEXT: &str = "text";

#[derive(Serialize, Deserialize)]
struct TextPayload {
    to: String,
    channel: String,
    body: String,
}

/// The `outbox` table, the key its payloads are sealed with, and a signal that wakes this
/// process's worker on enqueue.
#[derive(Clone)]
pub struct Outbox {
    pool: PgPool,
    crypto: Arc<dyn Crypto>,
    wake: Arc<Notify>,
}

impl Outbox {
    pub fn new(pool: PgPool, crypto: Arc<dyn Crypto>) -> Self {
        Self {
            pool,
            crypto,
            wake: Arc::new(Notify::new()),
        }
    }

    pub fn mailer(&self) -> OutboxMailer {
        OutboxMailer(self.clone())
    }

    pub fn texts(&self, channels: &[CodeChannel]) -> OutboxTexts {
        OutboxTexts {
            outbox: self.clone(),
            channels: channels.to_vec(),
        }
    }

    /// Spawns the worker that delivers through `mail` and `texts`, the real transports; must be
    /// called inside a Tokio runtime. Run one per instance: they share the table safely.
    pub fn start_worker(&self, mail: Arc<dyn Mailer>, texts: Arc<dyn TextSender>) -> OutboxWorker {
        let (stop, stopping) = watch::channel(false);
        let worker = Worker {
            outbox: self.clone(),
            mail,
            texts,
        };
        OutboxWorker {
            stop,
            handle: tokio::spawn(worker.run(stopping)),
        }
    }

    pub async fn pending(&self) -> Result<u64, StorageError> {
        let count = sqlx::query_scalar!(r#"select count(*) as "count!" from outbox"#)
            .fetch_one(&self.pool)
            .await
            .map_err(db_error)?;
        u64::try_from(count).map_err(corrupt)
    }

    async fn enqueue(
        &self,
        kind: &'static str,
        payload: &[u8],
        account: Option<&Email>,
        valid_for: time::Duration,
    ) -> Result<(), StorageError> {
        let id = Uuid::now_v7();
        let sealed = self
            .crypto
            .seal(payload, &associated_data(id, kind))
            .map_err(StorageError::backend)?;
        sqlx::query!(
            r#"
            insert into outbox (id, kind, payload, expires_at, user_id)
            values ($1, $2, $3, now() + make_interval(secs => $4),
                    (select id from users where lower(email) = lower($5)))
            "#,
            id,
            kind,
            sealed,
            valid_for.as_seconds_f64(),
            account.map(Email::as_str),
        )
        .execute(&self.pool)
        .await
        .map_err(db_error)?;
        self.wake.notify_one();
        Ok(())
    }

    fn open(&self, message: &Claimed) -> Result<Vec<u8>, String> {
        self.crypto
            .open(
                &message.payload,
                &associated_data(message.id, &message.kind),
            )
            .map(|plaintext| plaintext.to_vec())
            .map_err(|err| ErrorChain(&err).to_string())
    }
}

/// What a sealed payload is bound to: its row and kind, so a payload copied to another row, or
/// relabelled from text to mail, does not open.
fn associated_data(id: Uuid, kind: &str) -> Vec<u8> {
    format!("outbox:{kind}:{id}").into_bytes()
}

#[derive(Clone)]
pub struct OutboxMailer(Outbox);

impl Mailer for OutboxMailer {
    fn send(&self, mail: Mail) -> MailFuture<'_> {
        let payload = MailPayload {
            to: mail.to.as_str().to_owned(),
            template: mail.template,
            subject: mail.subject,
            body: mail.body,
        };
        let to = mail.to;
        let valid_for = mail.valid_for.unwrap_or(MAX_RETENTION).min(MAX_RETENTION);
        Box::pin(async move {
            let payload = serde_json::to_vec(&payload).map_err(MailError::new)?;
            self.0
                .enqueue(MAIL, &payload, Some(&to), valid_for)
                .await
                .map_err(MailError::new)
        })
    }
}

/// A [`TextSender`] that queues texts in the outbox instead of sending them.
#[derive(Clone)]
pub struct OutboxTexts {
    outbox: Outbox,
    channels: Vec<CodeChannel>,
}

impl TextSender for OutboxTexts {
    fn channels(&self) -> &[CodeChannel] {
        &self.channels
    }

    fn send(&self, message: TextMessage) -> TextFuture<'_> {
        let payload = TextPayload {
            to: message.to.as_str().to_owned(),
            channel: message.channel.as_str().to_owned(),
            body: message.body,
        };
        let valid_for = message.valid_for.min(MAX_RETENTION);
        Box::pin(async move {
            let payload = serde_json::to_vec(&payload).map_err(TextError::new)?;
            self.outbox
                .enqueue(TEXT, &payload, None, valid_for)
                .await
                .map_err(TextError::new)
        })
    }
}

/// The delivery task. [`OutboxWorker::finish`] stops it once it has delivered everything that is
/// due; wrap it in a timeout during shutdown.
pub struct OutboxWorker {
    stop: watch::Sender<bool>,
    handle: JoinHandle<()>,
}

impl OutboxWorker {
    pub async fn finish(self) {
        let _no_receiver = self.stop.send(true);
        if let Err(err) = self.handle.await {
            tracing::error!(error = %err, "outbox worker panicked");
        }
    }
}

struct Worker {
    outbox: Outbox,
    mail: Arc<dyn Mailer>,
    texts: Arc<dyn TextSender>,
}

struct Claimed {
    id: Uuid,
    kind: String,
    payload: Vec<u8>,
    attempts: i32,
}

impl Worker {
    async fn run(self, mut stopping: watch::Receiver<bool>) {
        loop {
            let delivered = match self.deliver_due().await {
                Ok(delivered) => delivered,
                Err(err) => {
                    tracing::warn!(error = %ErrorChain(&err), "outbox: claiming messages failed");
                    0
                }
            };
            if delivered > 0 {
                continue;
            }
            if *stopping.borrow() {
                return;
            }
            tokio::select! {
                () = self.outbox.wake.notified() => {}
                () = tokio::time::sleep(POLL) => {}
                _ = stopping.changed() => {}
            }
        }
    }

    /// Drops expired messages, then delivers one batch of due ones and returns how many it claimed.
    async fn deliver_due(&self) -> Result<usize, StorageError> {
        let expired = sqlx::query!("delete from outbox where expires_at <= now()")
            .execute(&self.outbox.pool)
            .await
            .map_err(db_error)?
            .rows_affected();
        if expired > 0 {
            tracing::warn!(expired, "outbox: dropped messages that expired undelivered");
        }
        let claimed = sqlx::query_as!(
            Claimed,
            r#"
            update outbox
            set available_at = now() + make_interval(secs => $1)
            where id in (
                select id from outbox
                where available_at <= now() and expires_at > now()
                order by available_at
                limit $2
                for update skip locked
            )
            returning id, kind, payload, attempts
            "#,
            LEASE_SECONDS,
            BATCH,
        )
        .fetch_all(&self.outbox.pool)
        .await
        .map_err(db_error)?;

        let count = claimed.len();
        for message in claimed {
            let id = message.id;
            let attempts = message.attempts + 1;
            let result = match timeout(DELIVERY_TIMEOUT, self.deliver(message)).await {
                Ok(result) => result,
                Err(_) => Err("delivery timed out".to_owned()),
            };
            match result {
                Ok(()) => self.delete(id).await?,
                Err(error) if attempts >= MAX_ATTEMPTS => {
                    tracing::error!(%id, attempts, error, "outbox: giving up on a message");
                    self.delete(id).await?;
                }
                Err(error) => {
                    tracing::warn!(%id, attempts, error, "outbox: delivery failed, will retry");
                    self.retry(id, attempts, &error).await?;
                }
            }
        }
        Ok(count)
    }

    async fn deliver(&self, message: Claimed) -> Result<(), String> {
        let payload = self.outbox.open(&message)?;
        match message.kind.as_str() {
            MAIL => {
                let payload: MailPayload =
                    serde_json::from_slice(&payload).map_err(|err| err.to_string())?;
                let mail = Mail {
                    to: Email::parse(&payload.to).map_err(|err| err.to_string())?,
                    template: payload.template,
                    subject: payload.subject,
                    body: payload.body,
                    valid_for: None,
                };
                self.mail
                    .send(mail)
                    .await
                    .map_err(|err| ErrorChain(&err).to_string())
            }
            TEXT => {
                let payload: TextPayload =
                    serde_json::from_slice(&payload).map_err(|err| err.to_string())?;
                let text = TextMessage {
                    to: PhoneNumber::parse(&payload.to).map_err(|err| err.to_string())?,
                    channel: CodeChannel::parse(&payload.channel).map_err(|err| err.to_string())?,
                    body: payload.body,
                    valid_for: MAX_RETENTION,
                };
                self.texts
                    .send(text)
                    .await
                    .map_err(|err| ErrorChain(&err).to_string())
            }
            other => Err(format!("unknown message kind `{other}`")),
        }
    }

    async fn delete(&self, id: Uuid) -> Result<(), StorageError> {
        sqlx::query!("delete from outbox where id = $1", id)
            .execute(&self.outbox.pool)
            .await
            .map_err(db_error)?;
        Ok(())
    }

    async fn retry(&self, id: Uuid, attempts: i32, error: &str) -> Result<(), StorageError> {
        let delay = 10.0 * 2f64.powi(attempts - 1).min(2160.0);
        sqlx::query!(
            r#"
            update outbox
            set attempts = $2, last_error = $3, available_at = now() + make_interval(secs => $4)
            where id = $1
            "#,
            id,
            attempts,
            error.chars().take(1000).collect::<String>(),
            delay,
        )
        .execute(&self.outbox.pool)
        .await
        .map_err(db_error)?;
        Ok(())
    }
}
