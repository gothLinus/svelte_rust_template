use domain::{
    clock::Clock,
    mail::{Mail, Mailer},
    user::Email,
};
use infrastructure::testing::{ManualClock, RecordingMailer};
use time::{Duration, macros::datetime};

#[test]
fn the_manual_clock_moves_only_when_told() {
    let clock = ManualClock::new(datetime!(2026-01-01 00:00:00.123_456_789 UTC));
    assert_eq!(clock.now(), datetime!(2026-01-01 00:00:00.123_456 UTC));

    clock.advance(Duration::hours(1));
    assert_eq!(clock.now(), datetime!(2026-01-01 01:00:00.123_456 UTC));

    clock.set(datetime!(2030-01-01 00:00 UTC));
    assert_eq!(clock.clone().now(), datetime!(2030-01-01 00:00 UTC));
    assert!(ManualClock::starting_now().now() > datetime!(2026-01-01 00:00 UTC));
}

#[tokio::test]
async fn the_recording_mailer_keeps_everything() {
    let mailer = RecordingMailer::new();
    for to in ["a@example.com", "b@example.com", "a@example.com"] {
        mailer
            .send(Mail {
                to: Email::parse(to).unwrap(),
                template: "test".to_owned(),
                subject: format!("to {to}"),
                body: String::new(),
                valid_for: None,
            })
            .await
            .unwrap();
    }

    assert_eq!(mailer.sent().len(), 3);
    assert!(mailer.last_to("b@example.com").is_some());
    assert!(mailer.last_to("c@example.com").is_none());
    assert_eq!(mailer.take().len(), 3);
    assert!(mailer.sent().is_empty());
}
