use std::{sync::Arc, time::Duration};

use domain::{
    mail::{Mail, MailError, MailFuture, Mailer},
    secret::Secret,
    user::Email,
};
use infrastructure::{
    config::{MailConfig, MailTransport},
    mail::{self, LogMailer, MailQueue, QUEUE_CAPACITY, SmtpMailer},
    testing::RecordingMailer,
};
use tokio::sync::Notify;

fn mail(to: &str) -> Mail {
    Mail {
        to: Email::parse(to).unwrap(),
        template: "test".to_owned(),
        subject: "Hello".to_owned(),
        body: "secret link".to_owned(),
        valid_for: None,
    }
}

#[test]
fn mail_debug_hides_the_body_and_the_address() {
    let debug = format!("{:?}", mail("alice@example.com"));
    assert!(!debug.contains("secret link"));
    assert!(!debug.contains("alice@"));
}

#[tokio::test]
async fn the_queue_delivers_in_the_background_and_drains_on_shutdown() {
    let transport = RecordingMailer::new();
    let (queue, worker) = MailQueue::start(Arc::new(transport.clone()));

    queue.send(mail("a@example.com")).await.unwrap();
    queue.send(mail("b@example.com")).await.unwrap();
    drop(queue);
    worker.finish().await;

    let sent: Vec<String> = transport.sent().iter().map(|m| m.to.to_string()).collect();
    assert_eq!(sent, ["a@example.com", "b@example.com"]);
}

struct Stuck(Arc<Notify>);

impl Mailer for Stuck {
    fn send(&self, _: Mail) -> MailFuture<'_> {
        let release = Arc::clone(&self.0);
        Box::pin(async move {
            release.notified().await;
            Ok(())
        })
    }
}

#[tokio::test]
async fn a_full_queue_refuses_mail_instead_of_blocking() {
    let (queue, _worker) = MailQueue::start(Arc::new(Stuck(Arc::new(Notify::new()))));

    let mut refused = 0;
    for _ in 0..=QUEUE_CAPACITY + 1 {
        if queue.send(mail("a@example.com")).await.is_err() {
            refused += 1;
        }
    }
    assert!(refused >= 1);
}

struct Failing;

impl Mailer for Failing {
    fn send(&self, _: Mail) -> MailFuture<'_> {
        Box::pin(async { Err(MailError::new(std::io::Error::other("relay down"))) })
    }
}

#[tokio::test]
async fn delivery_failures_are_logged_not_propagated() {
    let (queue, worker) = MailQueue::start(Arc::new(Failing));
    queue.send(mail("a@example.com")).await.unwrap();
    drop(queue);
    tokio::time::timeout(Duration::from_secs(5), worker.finish())
        .await
        .unwrap();
}

#[tokio::test]
async fn the_log_transport_accepts_everything() {
    LogMailer.send(mail("a@example.com")).await.unwrap();
}

#[tokio::test]
async fn transports_are_built_from_config() {
    let from: lettre::message::Mailbox = "App <noreply@example.com>".parse().unwrap();

    let log = MailConfig {
        transport: MailTransport::Log,
        from: from.clone(),
    };
    let (mailer, worker) = mail::start(&log).unwrap();
    mailer.send(mail("a@example.com")).await.unwrap();
    drop(mailer);
    worker.finish().await;

    let smtp = MailConfig {
        transport: MailTransport::Smtp {
            url: Secret::new("smtp://localhost:1"),
        },
        from: from.clone(),
    };
    let (_mailer, _worker) = mail::start(&smtp).unwrap();

    assert!(SmtpMailer::new(&Secret::new("http://not-smtp"), from).is_err());
}
