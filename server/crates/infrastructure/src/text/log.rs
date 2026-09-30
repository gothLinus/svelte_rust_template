use domain::{
    one_time_code::CodeChannel,
    text::{TextFuture, TextMessage, TextSender},
};

/// Writes text messages to the log instead of sending them. The body, which holds a sign-in code,
/// is logged at `debug` under the `text` target, like mail under `mail`. Never use this in
/// production.
#[derive(Debug, Clone, Copy, Default)]
pub struct LogTextSender;

impl TextSender for LogTextSender {
    fn channels(&self) -> &[CodeChannel] {
        &[CodeChannel::Sms, CodeChannel::Whatsapp]
    }

    fn send(&self, message: TextMessage) -> TextFuture<'_> {
        tracing::info!(
            target: "text",
            to = %message.to.masked(),
            channel = %message.channel,
            "text message not sent (TEXT_TRANSPORT=log)"
        );
        tracing::debug!(target: "text", to = %message.to, body = %message.body, "text body");
        Box::pin(async { Ok(()) })
    }
}
