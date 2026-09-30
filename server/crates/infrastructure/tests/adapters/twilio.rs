use std::sync::{Arc, Mutex};

use domain::{
    error::ErrorChain,
    one_time_code::CodeChannel,
    secret::Secret,
    text::{TextMessage, TextSender},
    user::PhoneNumber,
};
use infrastructure::{config::TwilioConfig, text::TwilioTextSender};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

async fn serve_once(response: &'static str) -> (String, Arc<Mutex<String>>) {
    let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
    let base = format!("http://{}", listener.local_addr().unwrap());
    let received = Arc::new(Mutex::new(String::new()));
    let sink = Arc::clone(&received);
    tokio::spawn(async move {
        let (mut stream, _) = listener.accept().await.unwrap();
        let mut request = Vec::new();
        let mut buffer = [0u8; 4096];
        loop {
            let read = stream.read(&mut buffer).await.unwrap();
            request.extend_from_slice(&buffer[..read]);
            let text = String::from_utf8_lossy(&request);
            if let Some((head, body)) = text.split_once("\r\n\r\n") {
                let length: usize = head
                    .lines()
                    .find_map(|line| line.strip_prefix("content-length: "))
                    .map_or(0, |value| value.trim().parse().unwrap());
                if body.len() >= length {
                    break;
                }
            }
            if read == 0 {
                break;
            }
        }
        *sink.lock().unwrap() = String::from_utf8_lossy(&request).into_owned();
        stream.write_all(response.as_bytes()).await.unwrap();
    });
    (base, received)
}

fn sender(sms: bool, whatsapp: bool, base: &str) -> TwilioTextSender {
    TwilioTextSender::new(
        TwilioConfig {
            account_sid: "AC123".to_owned(),
            auth_token: Secret::new("token"),
            sms_from: sms.then(|| PhoneNumber::parse("+15550001111").unwrap()),
            whatsapp_from: whatsapp.then(|| PhoneNumber::parse("+15550002222").unwrap()),
        },
        infrastructure::oauth::http_client().unwrap(),
    )
    .with_api_base(base)
}

fn message(channel: CodeChannel) -> TextMessage {
    TextMessage {
        to: PhoneNumber::parse("+491701234567").unwrap(),
        channel,
        body: "123 456 is your code".to_owned(),
        valid_for: domain::one_time_code::CODE_TTL,
    }
}

#[tokio::test]
async fn sms_goes_to_the_messages_endpoint_with_basic_auth() {
    let (base, received) = serve_once("HTTP/1.1 201 Created\r\ncontent-length: 2\r\n\r\n{}").await;
    let sender = sender(true, false, &base);
    assert_eq!(sender.channels(), [CodeChannel::Sms]);

    sender.send(message(CodeChannel::Sms)).await.unwrap();

    let request = received.lock().unwrap().clone();
    assert!(
        request.starts_with("POST /Accounts/AC123/Messages.json "),
        "{request}"
    );
    assert!(
        request.contains("authorization: Basic QUMxMjM6dG9rZW4="),
        "{request}"
    );
    assert!(request.contains("To=%2B491701234567"), "{request}");
    assert!(request.contains("From=%2B15550001111"), "{request}");
    assert!(request.contains("Body=123+456+is+your+code"), "{request}");
}

#[tokio::test]
async fn whatsapp_numbers_are_prefixed() {
    let (base, received) = serve_once("HTTP/1.1 201 Created\r\ncontent-length: 2\r\n\r\n{}").await;
    sender(false, true, &base)
        .send(message(CodeChannel::Whatsapp))
        .await
        .unwrap();
    let request = received.lock().unwrap().clone();
    assert!(
        request.contains("To=whatsapp%3A%2B491701234567"),
        "{request}"
    );
    assert!(
        request.contains("From=whatsapp%3A%2B15550002222"),
        "{request}"
    );
}

#[tokio::test]
async fn refusals_are_errors_with_the_number_masked() {
    let (base, _) = serve_once(
        "HTTP/1.1 400 Bad Request\r\ncontent-length: 67\r\n\r\n{\"code\":21211,\"message\":\"The 'To' number +491701234567 is not valid.\"}",
    )
    .await;
    let err = sender(true, false, &base)
        .send(message(CodeChannel::Sms))
        .await
        .unwrap_err();
    let text = ErrorChain(&err).to_string();
    assert!(text.contains("400"), "{text}");
    assert!(!text.contains("+491701234567"), "{text}");
    assert!(text.contains("+49"), "{text}");
}

#[tokio::test]
async fn channels_without_a_sender_and_email_are_refused() {
    let sender = sender(true, false, "http://127.0.0.1:9");
    assert!(sender.send(message(CodeChannel::Whatsapp)).await.is_err());
    assert!(sender.send(message(CodeChannel::Email)).await.is_err());
}
