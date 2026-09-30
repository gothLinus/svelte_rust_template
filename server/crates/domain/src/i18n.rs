//! Text that reaches a person, without the words.
//!
//! The domain and the use cases never contain user-facing English. They name a [`Message`] (a
//! key of the catalog in `/locales`) and hand over its values ([`Arg`]); a [`Translator`] turns
//! that into a sentence in the reader's [`Locale`]. Tests ask the same translator for the text
//! they expect instead of repeating it.

use std::{borrow::Cow, fmt};

/// A language tag such as `en` or `pt-BR`, as the catalog names its directory.
///
/// Only a name: whether the catalog has that language is for the [`Translator`] to say.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Locale(Cow<'static, str>);

impl Locale {
    pub const EN: Self = Self(Cow::Borrowed("en"));

    /// `None` unless `tag` is a plausible BCP 47 tag: ASCII letters, digits and dashes, starting
    /// with a letter.
    pub fn parse(tag: &str) -> Option<Self> {
        let valid = !tag.is_empty()
            && tag.len() <= 35
            && tag.starts_with(|c: char| c.is_ascii_alphabetic())
            && tag.chars().all(|c| c.is_ascii_alphanumeric() || c == '-');
        valid.then(|| Self(Cow::Owned(tag.to_owned())))
    }

    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl Default for Locale {
    fn default() -> Self {
        Self::EN
    }
}

impl fmt::Display for Locale {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Arg {
    Int(i64),
    Text(String),
}

impl From<i64> for Arg {
    fn from(value: i64) -> Self {
        Self::Int(value)
    }
}

impl From<i32> for Arg {
    fn from(value: i32) -> Self {
        Self::Int(value.into())
    }
}

impl From<u32> for Arg {
    fn from(value: u32) -> Self {
        Self::Int(value.into())
    }
}

impl From<usize> for Arg {
    fn from(value: usize) -> Self {
        Self::Int(i64::try_from(value).unwrap_or(i64::MAX))
    }
}

impl From<&str> for Arg {
    fn from(value: &str) -> Self {
        Self::Text(value.to_owned())
    }
}

impl From<String> for Arg {
    fn from(value: String) -> Self {
        Self::Text(value)
    }
}

/// What to say: a catalog key and the values for its placeholders.
///
/// ```
/// # use domain::i18n::Message;
/// let message = Message::new("validation-too-long").arg("max", 100);
/// assert_eq!(message.id(), "validation-too-long");
/// ```
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Message {
    id: &'static str,
    args: Vec<(&'static str, Arg)>,
}

impl Message {
    pub fn new(id: &'static str) -> Self {
        Self {
            id,
            args: Vec::new(),
        }
    }

    #[must_use]
    pub fn arg(mut self, name: &'static str, value: impl Into<Arg>) -> Self {
        self.args.push((name, value.into()));
        self
    }

    pub fn id(&self) -> &'static str {
        self.id
    }

    pub fn args(&self) -> &[(&'static str, Arg)] {
        &self.args
    }
}

/// Turns a [`Message`] into text for a reader.
///
/// A message the locale lacks falls back to the default language, and one no language has comes
/// back as its id, so a missing translation is visible and never an error the reader sees.
pub trait Translator: Send + Sync + 'static {
    fn locales(&self) -> Vec<Locale>;

    fn negotiate(&self, accept_language: Option<&str>) -> Locale;

    fn translate(&self, locale: &Locale, message: &Message) -> String;
}
