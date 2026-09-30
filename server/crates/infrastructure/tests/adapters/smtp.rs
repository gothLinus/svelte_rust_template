use domain::{
    mail::{Mail, Mailer},
    secret::Secret,
    user::Email,
};
use infrastructure::mail::SmtpMailer;
use tokio::{
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::TcpListener,
    sync::oneshot,
};

async fn fake_smtp_server() -> (u16, oneshot::Receiver<String>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    let (sender, receiver) = oneshot::channel();

    tokio::spawn(async move {
        let (stream, _) = listener.accept().await.unwrap();
        let (read, mut write) = stream.into_split();
        let mut lines = BufReader::new(read).lines();
        write.write_all(b"220 fake ESMTP\r\n").await.unwrap();

        let mut data = String::new();
        while let Some(line) = lines.next_line().await.unwrap() {
            let command = line.to_ascii_uppercase();
            let reply: &[u8] = if command.starts_with("EHLO") || command.starts_with("HELO") {
                b"250-fake\r\n250 OK\r\n"
            } else if command.starts_with("DATA") {
                write.write_all(b"354 go ahead\r\n").await.unwrap();
                while let Some(line) = lines.next_line().await.unwrap() {
                    if line == "." {
                        break;
                    }
                    data.push_str(&line);
                    data.push('\n');
                }
                b"250 queued\r\n"
            } else if command.starts_with("QUIT") {
                write.write_all(b"221 bye\r\n").await.unwrap();
                break;
            } else {
                b"250 OK\r\n"
            };
            write.write_all(reply).await.unwrap();
        }
        let _ = sender.send(data);
    });

    (port, receiver)
}

#[tokio::test]
async fn smtp_delivers_a_plain_text_message() {
    let (port, received) = fake_smtp_server().await;
    let mailer = SmtpMailer::new(
        &Secret::new(format!("smtp://127.0.0.1:{port}")),
        "App <noreply@example.com>".parse().unwrap(),
    )
    .unwrap();

    mailer
        .send(Mail {
            to: Email::parse("alice@example.com").unwrap(),
            template: "test".to_owned(),
            subject: "Hello".to_owned(),
            body: "Hi Alice".to_owned(),
            valid_for: None,
        })
        .await
        .unwrap();
    drop(mailer);

    let data = received.await.unwrap();
    assert!(data.contains("To: alice@example.com"), "{data}");
    assert!(data.contains("From: App <noreply@example.com>"), "{data}");
    assert!(data.contains("Subject: Hello"), "{data}");
    assert!(data.contains("Content-Type: text/plain"), "{data}");
    assert!(data.contains("Hi Alice"), "{data}");
}

#[tokio::test]
async fn an_unreachable_relay_is_an_error() {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let port = listener.local_addr().unwrap().port();
    drop(listener);
    let mailer = SmtpMailer::new(
        &Secret::new(format!("smtp://127.0.0.1:{port}")),
        "App <noreply@example.com>".parse().unwrap(),
    )
    .unwrap();

    let result = mailer
        .send(Mail {
            to: Email::parse("alice@example.com").unwrap(),
            template: "test".to_owned(),
            subject: "Hello".to_owned(),
            body: String::new(),
            valid_for: None,
        })
        .await;

    assert!(result.is_err());
}
