use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use domain::{
    mail::{Mail, MailError, MailFuture, Mailer},
    one_time_code::CodeChannel,
    text::{TextMessage, TextSender},
    user::{Email, PhoneNumber},
};
use infrastructure::{
    crypto::RingCrypto,
    outbox::{MAX_ATTEMPTS, Outbox},
    testing::{RecordingMailer, RecordingTexts},
};
use sqlx::PgPool;

fn mail(to: &str) -> Mail {
    Mail {
        to: Email::parse(to).unwrap(),
        template: "test".to_owned(),
        subject: "Hello".to_owned(),
        body: "a link".to_owned(),
        valid_for: None,
    }
}

fn outbox(pool: &PgPool) -> Outbox {
    Outbox::new(pool.clone(), Arc::new(RingCrypto::new(&[3; 32])))
}

#[derive(Default)]
struct DownMailer(AtomicUsize);

#[derive(Debug, thiserror::Error)]
#[error("relay unreachable")]
struct Unreachable;

impl Mailer for DownMailer {
    fn send(&self, _mail: Mail) -> MailFuture<'_> {
        self.0.fetch_add(1, Ordering::SeqCst);
        Box::pin(async { Err(MailError::new(Unreachable)) })
    }
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn queued_messages_are_delivered_and_removed(pool: PgPool) {
    let outbox = outbox(&pool);
    let recorded = RecordingMailer::new();
    let texts = RecordingTexts::new();

    outbox
        .mailer()
        .send(mail("alice@example.com"))
        .await
        .unwrap();
    let sender = outbox.texts(&[CodeChannel::Sms]);
    assert_eq!(sender.channels(), [CodeChannel::Sms]);
    sender
        .send(TextMessage {
            to: PhoneNumber::parse("+15550001111").unwrap(),
            channel: CodeChannel::Sms,
            body: "123 456".to_owned(),
            valid_for: domain::one_time_code::CODE_TTL,
        })
        .await
        .unwrap();
    assert_eq!(outbox.pending().await.unwrap(), 2);

    let worker = outbox.start_worker(Arc::new(recorded.clone()), Arc::new(texts.clone()));
    worker.finish().await;

    assert_eq!(outbox.pending().await.unwrap(), 0);
    let sent = recorded.last_to("alice@example.com").unwrap();
    assert_eq!(
        (sent.template.as_str(), sent.body.as_str()),
        ("test", "a link")
    );
    assert_eq!(texts.last_code_to("+15550001111").unwrap(), "123456");
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn failed_deliveries_are_retried_later_then_dropped(pool: PgPool) {
    let outbox = outbox(&pool);
    outbox
        .mailer()
        .send(mail("alice@example.com"))
        .await
        .unwrap();
    let down = Arc::new(DownMailer::default());

    let worker = outbox.start_worker(down.clone(), Arc::new(RecordingTexts::new()));
    worker.finish().await;

    assert_eq!(down.0.load(Ordering::SeqCst), 1);
    let (attempts, error, later): (i32, Option<String>, bool) = sqlx::query_as(
        "select attempts, last_error, available_at > now() + interval '5 seconds' from outbox",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(attempts, 1);
    assert!(error.unwrap().contains("relay unreachable"));
    assert!(later);

    sqlx::query("update outbox set attempts = $1, available_at = now()")
        .bind(MAX_ATTEMPTS - 1)
        .execute(&pool)
        .await
        .unwrap();
    let worker = outbox.start_worker(down.clone(), Arc::new(RecordingTexts::new()));
    worker.finish().await;
    assert_eq!(down.0.load(Ordering::SeqCst), 2);
    assert_eq!(outbox.pending().await.unwrap(), 0);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn a_running_worker_delivers_what_is_enqueued(pool: PgPool) {
    let outbox = outbox(&pool);
    let recorded = RecordingMailer::new();
    let worker = outbox.start_worker(Arc::new(recorded.clone()), Arc::new(RecordingTexts::new()));

    outbox.mailer().send(mail("bob@example.com")).await.unwrap();
    for _ in 0..100 {
        if recorded.last_to("bob@example.com").is_some() {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    assert!(recorded.last_to("bob@example.com").is_some());
    worker.finish().await;
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn unreadable_messages_are_dropped(pool: PgPool) {
    let outbox = outbox(&pool);
    sqlx::query(
        "insert into outbox (id, kind, payload, attempts, expires_at) \
         values (gen_random_uuid(), 'mail', '\\x00', $1, now() + interval '1 hour')",
    )
    .bind(MAX_ATTEMPTS - 1)
    .execute(&pool)
    .await
    .unwrap();
    let worker = outbox.start_worker(
        Arc::new(RecordingMailer::new()),
        Arc::new(RecordingTexts::new()),
    );
    worker.finish().await;
    assert_eq!(outbox.pending().await.unwrap(), 0);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn payloads_are_sealed_and_bound_to_their_row(pool: PgPool) {
    let outbox = outbox(&pool);
    let mut secret = mail("alice@example.com");
    secret.body = "https://app.example.com/reset-password#token=raw-token".to_owned();
    outbox.mailer().send(secret).await.unwrap();
    outbox.mailer().send(mail("bob@example.com")).await.unwrap();

    let payloads: Vec<Vec<u8>> = sqlx::query_scalar("select payload from outbox order by id")
        .fetch_all(&pool)
        .await
        .unwrap();
    for payload in &payloads {
        let text = String::from_utf8_lossy(payload);
        assert!(
            !text.contains("raw-token") && !text.contains("example.com"),
            "{text}"
        );
    }

    sqlx::query("update outbox set payload = case when payload = $1 then $2 else $1 end")
        .bind(&payloads[0])
        .bind(&payloads[1])
        .execute(&pool)
        .await
        .unwrap();
    let recorded = RecordingMailer::new();
    outbox
        .start_worker(Arc::new(recorded.clone()), Arc::new(RecordingTexts::new()))
        .finish()
        .await;
    assert!(recorded.last_to("alice@example.com").is_none());
    assert!(recorded.last_to("bob@example.com").is_none());
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn messages_expire_with_what_they_carry(pool: PgPool) {
    let outbox = outbox(&pool);
    let mut link = mail("alice@example.com");
    link.valid_for = Some(time::Duration::minutes(15));
    outbox.mailer().send(link).await.unwrap();
    outbox.mailer().send(mail("bob@example.com")).await.unwrap();

    let (link_ttl, notice_ttl): (f64, f64) = sqlx::query_as(
        "select min(extract(epoch from expires_at - created_at))::float8, \
                max(extract(epoch from expires_at - created_at))::float8 from outbox",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!((link_ttl - 15.0 * 60.0).abs() < 5.0, "{link_ttl}");
    assert!((notice_ttl - 2.0 * 86400.0).abs() < 5.0, "{notice_ttl}");

    sqlx::query("update outbox set expires_at = now() - interval '1 second'")
        .execute(&pool)
        .await
        .unwrap();
    let recorded = RecordingMailer::new();
    outbox
        .start_worker(Arc::new(recorded.clone()), Arc::new(RecordingTexts::new()))
        .finish()
        .await;
    assert!(recorded.sent().is_empty());
    assert_eq!(outbox.pending().await.unwrap(), 0);
}

#[sqlx::test(migrator = "infrastructure::db::MIGRATOR")]
async fn deleting_an_account_deletes_its_undelivered_mail(pool: PgPool) {
    sqlx::query(
        "insert into users (id, email, username) values (gen_random_uuid(), 'alice@example.com', 'alice')",
    )
    .execute(&pool)
    .await
    .unwrap();
    let outbox = outbox(&pool);
    outbox
        .mailer()
        .send(mail("Alice@Example.com"))
        .await
        .unwrap();
    outbox
        .mailer()
        .send(mail("stranger@example.com"))
        .await
        .unwrap();
    assert_eq!(outbox.pending().await.unwrap(), 2);

    sqlx::query("delete from users where email = 'alice@example.com'")
        .execute(&pool)
        .await
        .unwrap();

    assert_eq!(outbox.pending().await.unwrap(), 1);
}
