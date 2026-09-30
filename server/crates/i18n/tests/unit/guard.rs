//! Keeps the code and the catalog in step, so that neither a missing translation nor a dead one
//! survives to production:
//!
//! - every message id the server's code names exists in `locales/en/server`, and is given exactly
//!   the values (`.arg`) its text reads (`{ $name }`);
//! - every English message is used by the code somewhere;
//! - every other language has the same messages, with the same placeholders, as English.
//!
//! The code is found by reading `crates/*/src/**/*.rs` as text: `Message::new("id")`, and any
//! other string literal that is shaped like an id of the catalog (a `match` that picks the id,
//! say), which counts as a use too. Comments are skipped.

use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
};

use domain::i18n::{Locale, Translator};
use i18n::Catalog;

/// Ids no source file names, on purpose. Add one only with the reason; a message that nothing
/// uses should be deleted instead.
const UNUSED_ALLOWED: &[(&str, &str)] = &[];

#[derive(Debug)]
struct Use {
    id: String,
    at: String,
    args: Option<Vec<String>>,
}

fn server_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn rust_files(dir: &Path, out: &mut Vec<PathBuf>) {
    for entry in fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        if path.is_dir() {
            rust_files(&path, out);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            out.push(path);
        }
    }
}

fn sources() -> Vec<PathBuf> {
    let mut files = Vec::new();
    for krate in fs::read_dir(server_dir().join("crates")).unwrap() {
        let src = krate.unwrap().path().join("src");
        if src.is_dir() {
            rust_files(&src, &mut files);
        }
    }
    files.sort();
    files
}

fn without_comments(source: &str) -> String {
    source
        .lines()
        .map(|line| {
            if line.trim_start().starts_with("//") {
                ""
            } else {
                line
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn literal_at(text: &str, start: usize) -> Option<(String, usize)> {
    let bytes = text.as_bytes();
    if bytes.get(start) != Some(&b'"') {
        return None;
    }
    let mut end = start + 1;
    while end < bytes.len() && bytes[end] != b'"' {
        end += if bytes[end] == b'\\' { 2 } else { 1 };
    }
    Some((text[start + 1..end.min(text.len())].to_owned(), end + 1))
}

fn looks_like_an_id(text: &str) -> bool {
    text.contains('-')
        && text.split('-').all(|word| {
            !word.is_empty()
                && word
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit())
        })
}

fn chained_args(text: &str, at: usize) -> Vec<String> {
    let mut names = Vec::new();
    let rest = text[at..].trim_start();
    let mut at = text.len() - rest.len();
    if rest.starts_with(')') {
        at += 1;
    } else {
        return names;
    }
    loop {
        let rest = text[at..].trim_start();
        let skipped = text.len() - at - rest.len();
        if !rest.starts_with(".arg(") {
            return names;
        }
        let start = at + skipped + ".arg(".len();
        let Some((name, _)) = literal_at(text, start) else {
            return names;
        };
        names.push(name);
        // Skip to the parenthesis that closes this `.arg(`, over nested calls and strings.
        let mut depth = 1;
        let mut position = start;
        while depth > 0 && position < text.len() {
            if let Some((_, next)) = literal_at(text, position) {
                position = next;
                continue;
            }
            match text.as_bytes()[position] {
                b'(' => depth += 1,
                b')' => depth -= 1,
                _ => {}
            }
            position += 1;
        }
        at = position;
    }
}

/// Every literal id in the server's code. `areas` are the first words of the catalog's ids: a
/// literal that starts with one is taken for an id, so a misspelled one is caught as well as a
/// missing one.
fn uses(areas: &BTreeSet<String>) -> Vec<Use> {
    let mut found = Vec::new();
    for path in sources() {
        let text = without_comments(&fs::read_to_string(&path).unwrap());
        let file = path
            .strip_prefix(server_dir())
            .unwrap()
            .display()
            .to_string();
        let mut position = 0;
        while position < text.len() {
            let Some(offset) = text[position..].find('"') else {
                break;
            };
            let start = position + offset;
            let Some((content, next)) = literal_at(&text, start) else {
                break;
            };
            let call = text[..start].trim_end().ends_with("Message::new(");
            let area = content.split('-').next().unwrap_or_default();
            if call || (looks_like_an_id(&content) && areas.contains(area)) {
                let line = text[..start].matches('\n').count() + 1;
                found.push(Use {
                    args: call.then(|| chained_args(&text, next)),
                    id: content,
                    at: format!("{file}:{line}"),
                });
            }
            position = next;
        }
    }
    found
}

fn english() -> (Catalog, Locale) {
    (Catalog::embedded().unwrap(), Locale::EN)
}

fn areas(ids: &[String]) -> BTreeSet<String> {
    ids.iter()
        .filter_map(|id| id.split('-').next())
        .map(str::to_owned)
        .collect()
}

#[test]
fn the_scan_finds_the_code() {
    // A guard that reads nothing guards nothing.
    let (catalog, en) = english();
    let found = uses(&areas(&catalog.ids(&en)));
    assert!(found.len() > 50, "only {} uses found", found.len());
    assert!(found.iter().any(|used| used.id == "error-not-found"));
}

#[test]
fn every_id_the_code_uses_exists_with_the_values_its_text_reads() {
    let (catalog, en) = english();
    let ids = catalog.ids(&en);
    let mut problems = Vec::new();
    for used in uses(&areas(&ids)) {
        if ids.binary_search(&used.id).is_err() {
            problems.push(format!(
                "{}: `{}` is not in locales/en/server",
                used.at, used.id
            ));
            continue;
        }
        if let Some(mut args) = used.args {
            args.sort();
            args.dedup();
            let variables = catalog.variables(&en, &used.id);
            if args != variables {
                problems.push(format!(
                    "{}: `{}` is given {args:?} but its text reads {variables:?}",
                    used.at, used.id
                ));
            }
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn every_english_message_is_used_by_the_code() {
    let (catalog, en) = english();
    let ids = catalog.ids(&en);
    let used: BTreeSet<String> = uses(&areas(&ids)).into_iter().map(|used| used.id).collect();
    let allowed: BTreeSet<&str> = UNUSED_ALLOWED.iter().map(|(id, _)| *id).collect();
    let unused: Vec<&String> = ids
        .iter()
        .filter(|id| !used.contains(*id) && !allowed.contains(id.as_str()))
        .collect();
    assert!(
        unused.is_empty(),
        "no code names these messages, delete them or use them: {unused:?}"
    );
    for (id, reason) in UNUSED_ALLOWED {
        assert!(!reason.is_empty(), "say why `{id}` may stay unused");
        assert!(
            ids.iter().any(|known| known == id),
            "`{id}` is not a message"
        );
    }
}

#[test]
fn every_language_has_the_messages_and_placeholders_of_english() {
    let (catalog, en) = english();
    let english_ids = catalog.ids(&en);
    let variables: BTreeMap<&String, Vec<String>> = english_ids
        .iter()
        .map(|id| (id, catalog.variables(&en, id)))
        .collect();

    // Every directory of /locales that has server files is a language of the catalog.
    let mut on_disk: Vec<String> = fs::read_dir(server_dir().join("../locales"))
        .unwrap()
        .map(|entry| entry.unwrap().path())
        .filter(|path| path.join("server").is_dir())
        .map(|path| path.file_name().unwrap().to_string_lossy().into_owned())
        .collect();
    on_disk.sort();
    let mut embedded: Vec<String> = catalog
        .locales()
        .iter()
        .map(|locale| locale.as_str().to_owned())
        .collect();
    embedded.sort();
    assert_eq!(
        on_disk, embedded,
        "the build embeds every language directory"
    );

    let mut problems = Vec::new();
    for locale in catalog.locales().iter().filter(|locale| **locale != en) {
        let ids = catalog.ids(locale);
        for id in &english_ids {
            if ids.binary_search(id).is_err() {
                problems.push(format!("{locale}: `{id}` is missing"));
            } else if catalog.variables(locale, id) != variables[id] {
                problems.push(format!(
                    "{locale}: `{id}` reads {:?}, English reads {:?}",
                    catalog.variables(locale, id),
                    variables[id]
                ));
            }
        }
        for id in ids.iter().filter(|id| !variables.contains_key(id)) {
            problems.push(format!("{locale}: `{id}` does not exist in English"));
        }
    }
    assert!(problems.is_empty(), "\n{}", problems.join("\n"));
}

#[test]
fn the_parity_check_notices_differences() {
    let catalog = Catalog::from_sources(&[
        ("en", vec![("a.ftl", "one = { $a }\ntwo = { $b }\n")]),
        ("de", vec![("a.ftl", "one = { $b }\nthree = x\n")]),
    ])
    .unwrap();
    let de = Locale::parse("de").unwrap();
    assert_ne!(catalog.ids(&de), catalog.ids(&Locale::EN));
    assert_ne!(
        catalog.variables(&de, "one"),
        catalog.variables(&Locale::EN, "one")
    );
}
