//! The translation catalog, behind the domain's [`Translator`] port.
//!
//! The words live in `/locales/<language>/server/*.ftl`, written in
//! [Fluent](https://projectfluent.org): a translator can express plurals and grammar in their
//! language without touching code. The files are embedded at build time (see `build.rs`);
//! [`Catalog::embedded`] parses them once at startup and refuses to start on a file that does not
//! parse, so a typo never reaches a user.
//!
//! Tests use the same catalog to say what they expect (`catalog.text("error-not-found")`), which
//! keeps them independent of the wording and of the language.

use std::{collections::BTreeMap, fmt, sync::Arc};

use domain::i18n::{Arg, Locale, Message, Translator};
use fluent_bundle::{FluentArgs, FluentResource, FluentValue, concurrent::FluentBundle};
use fluent_langneg::{NegotiationStrategy, accepted_languages, negotiate_languages};
use unic_langid::LanguageIdentifier;

mod introspect;

mod embedded {
    include!(concat!(env!("OUT_DIR"), "/catalog.rs"));
}

#[derive(Debug, PartialEq, Eq)]
pub struct CatalogError(pub Vec<String>);

impl fmt::Display for CatalogError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("invalid translation catalog:")?;
        for problem in &self.0 {
            write!(f, "\n  - {problem}")?;
        }
        Ok(())
    }
}

impl std::error::Error for CatalogError {}

struct Language {
    bundle: FluentBundle<Arc<FluentResource>>,
    resources: Vec<(String, Arc<FluentResource>)>,
}

pub struct Catalog {
    languages: BTreeMap<String, Language>,
}

pub const DEFAULT_LOCALE: Locale = Locale::EN;

impl Catalog {
    pub fn embedded() -> Result<Self, CatalogError> {
        let sources: Vec<(&str, Vec<(&str, &str)>)> = embedded::EMBEDDED
            .iter()
            .map(|(language, files)| (*language, files.to_vec()))
            .collect();
        Self::from_sources(&sources)
    }

    /// The embedded catalog with further languages on top, for tests of what a second language does
    /// before one is shipped. A language named like an embedded one replaces it.
    pub fn embedded_with(extra: &[(&str, Vec<(&str, &str)>)]) -> Result<Self, CatalogError> {
        let mut sources: Vec<(&str, Vec<(&str, &str)>)> = embedded::EMBEDDED
            .iter()
            .filter(|(language, _)| !extra.iter().any(|(added, _)| added == language))
            .map(|(language, files)| (*language, files.to_vec()))
            .collect();
        sources.extend(extra.iter().cloned());
        Self::from_sources(&sources)
    }

    /// Builds a catalog from `(language, [(file name, Fluent source)])`. The default language must
    /// be among them. Duplicate ids within a language, syntax errors and unparsable language tags
    /// are all errors.
    pub fn from_sources(sources: &[(&str, Vec<(&str, &str)>)]) -> Result<Self, CatalogError> {
        let mut problems = Vec::new();
        let mut languages = BTreeMap::new();
        for (tag, files) in sources {
            let Ok(id) = tag.parse::<LanguageIdentifier>() else {
                problems.push(format!("`{tag}` is not a language tag"));
                continue;
            };
            let mut bundle = FluentBundle::new_concurrent(vec![id]);
            // Plain text, not UI: no invisible direction marks around placeholders.
            bundle.set_use_isolating(false);
            let mut resources = Vec::new();
            for (name, source) in files {
                match FluentResource::try_new((*source).to_owned()) {
                    Ok(resource) => resources.push(((*name).to_owned(), Arc::new(resource))),
                    Err((resource, errors)) => {
                        problems.push(format!("{tag}/{name}: {errors:?}"));
                        resources.push(((*name).to_owned(), Arc::new(resource)));
                    }
                }
            }
            for (name, resource) in &resources {
                if let Err(errors) = bundle.add_resource(Arc::clone(resource)) {
                    problems.push(format!("{tag}/{name}: {errors:?}"));
                }
            }
            languages.insert((*tag).to_owned(), Language { bundle, resources });
        }
        if !languages.contains_key(DEFAULT_LOCALE.as_str()) {
            problems.push(format!(
                "the default language `{DEFAULT_LOCALE}` is missing"
            ));
        }
        if problems.is_empty() {
            Ok(Self { languages })
        } else {
            Err(CatalogError(problems))
        }
    }

    pub fn text(&self, message: &Message) -> String {
        self.translate(&DEFAULT_LOCALE, message)
    }

    pub fn text_of(&self, id: &'static str) -> String {
        self.text(&Message::new(id))
    }

    pub fn ids(&self, locale: &Locale) -> Vec<String> {
        self.language(locale)
            .map(|language| introspect::ids(&language.resources))
            .unwrap_or_default()
    }

    pub fn variables(&self, locale: &Locale, id: &str) -> Vec<String> {
        self.language(locale)
            .map(|language| introspect::variables(&language.resources, id))
            .unwrap_or_default()
    }

    fn language(&self, locale: &Locale) -> Option<&Language> {
        self.languages.get(locale.as_str())
    }

    fn resolve(&self, locale: &Locale) -> Option<&Language> {
        self.language(locale).or_else(|| {
            let base = locale.as_str().split('-').next()?;
            self.languages.get(base)
        })
    }

    fn format(language: &Language, message: &Message) -> Option<String> {
        let pattern = language.bundle.get_message(message.id())?.value()?;
        let mut args = FluentArgs::new();
        for (name, value) in message.args() {
            args.set(
                *name,
                match value {
                    Arg::Int(number) => FluentValue::from(*number),
                    Arg::Text(text) => FluentValue::from(text.as_str()),
                },
            );
        }
        let mut errors = Vec::new();
        let text = language
            .bundle
            .format_pattern(pattern, Some(&args), &mut errors);
        if !errors.is_empty() {
            tracing::error!(id = message.id(), ?errors, "translation failed to format");
        }
        Some(text.into_owned())
    }
}

impl Translator for Catalog {
    fn locales(&self) -> Vec<Locale> {
        let mut locales = vec![DEFAULT_LOCALE];
        locales.extend(
            self.languages
                .keys()
                .filter(|tag| tag.as_str() != DEFAULT_LOCALE.as_str())
                .filter_map(|tag| Locale::parse(tag)),
        );
        locales
    }

    fn negotiate(&self, accept_language: Option<&str>) -> Locale {
        let Some(header) = accept_language else {
            return DEFAULT_LOCALE;
        };
        let requested = accepted_languages::parse(header);
        let available: Vec<LanguageIdentifier> = self
            .languages
            .keys()
            .filter_map(|tag| tag.parse().ok())
            .collect();
        let default: LanguageIdentifier = DEFAULT_LOCALE
            .as_str()
            .parse()
            .unwrap_or_else(|_| LanguageIdentifier::default());
        negotiate_languages(
            &requested,
            &available,
            Some(&default),
            NegotiationStrategy::Filtering,
        )
        .first()
        .and_then(|best| Locale::parse(&best.to_string()))
        .unwrap_or(DEFAULT_LOCALE)
    }

    fn translate(&self, locale: &Locale, message: &Message) -> String {
        let text = self
            .resolve(locale)
            .and_then(|language| Self::format(language, message))
            .or_else(|| {
                self.language(&DEFAULT_LOCALE)
                    .and_then(|language| Self::format(language, message))
            });
        text.unwrap_or_else(|| {
            tracing::error!(id = message.id(), %locale, "translation is missing");
            message.id().to_owned()
        })
    }
}
