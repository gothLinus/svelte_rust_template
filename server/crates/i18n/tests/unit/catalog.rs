use domain::i18n::{Locale, Message, Translator};
use i18n::{Catalog, DEFAULT_LOCALE};

const EN: &str = "greeting = Hello, { $name }!\nitems = { $count ->\n    [one] one item\n   *[other] { $count } items\n}\nonly-en = English only\n";
const DE: &str = "greeting = Hallo, { $name }!\nitems = { $count ->\n    [one] ein Eintrag\n   *[other] { $count } Einträge\n}\n";

fn catalog() -> Catalog {
    Catalog::from_sources(&[("en", vec![("a.ftl", EN)]), ("de", vec![("a.ftl", DE)])]).unwrap()
}

fn de() -> Locale {
    Locale::parse("de").unwrap()
}

#[test]
fn the_embedded_catalog_is_valid() {
    let catalog = Catalog::embedded().unwrap();
    assert_eq!(catalog.locales()[0], DEFAULT_LOCALE);
}

#[test]
fn interpolates_values_and_picks_plural_forms() {
    let catalog = catalog();
    let greeting = Message::new("greeting").arg("name", "Ada");
    assert_eq!(catalog.text(&greeting), "Hello, Ada!");
    assert_eq!(catalog.translate(&de(), &greeting), "Hallo, Ada!");

    let one = Message::new("items").arg("count", 1);
    let many = Message::new("items").arg("count", 3);
    assert_eq!(catalog.translate(&de(), &one), "ein Eintrag");
    assert_eq!(catalog.translate(&de(), &many), "3 Einträge");
}

#[test]
fn falls_back_to_the_default_language_and_then_to_the_id() {
    let catalog = catalog();
    assert_eq!(
        catalog.translate(&de(), &Message::new("only-en")),
        "English only"
    );
    assert_eq!(
        catalog.translate(&de(), &Message::new("nobody-has-this")),
        "nobody-has-this"
    );
    let unknown = Locale::parse("fr").unwrap();
    assert_eq!(
        catalog.translate(&unknown, &Message::new("only-en")),
        "English only"
    );
}

#[test]
fn a_regional_tag_reads_its_base_language() {
    let catalog = catalog();
    let regional = Locale::parse("de-AT").unwrap();
    assert_eq!(
        catalog.translate(&regional, &Message::new("greeting").arg("name", "Ada")),
        "Hallo, Ada!"
    );
}

#[test]
fn negotiates_accept_language() {
    let catalog = catalog();
    assert_eq!(catalog.negotiate(None), Locale::EN);
    assert_eq!(catalog.negotiate(Some("de-DE,de;q=0.9,en;q=0.8")), de());
    assert_eq!(catalog.negotiate(Some("fr, en;q=0.5")), Locale::EN);
    assert_eq!(catalog.negotiate(Some("fr")), Locale::EN);
    assert_eq!(catalog.negotiate(Some("not a header ;;")), Locale::EN);
}

#[test]
fn lists_locales_default_first() {
    assert_eq!(catalog().locales(), vec![Locale::EN, de()]);
}

#[test]
fn reports_every_problem_at_once() {
    let error = Catalog::from_sources(&[
        (
            "en",
            vec![
                ("a.ftl", "ok = fine\nok = twice\n"),
                ("b.ftl", "broken = {\n"),
            ],
        ),
        ("x y", vec![]),
    ])
    .err()
    .unwrap();
    assert_eq!(error.0.len(), 3, "{error}");
}

#[test]
fn a_catalog_needs_the_default_language() {
    let error = Catalog::from_sources(&[("de", vec![("a.ftl", DE)])])
        .err()
        .unwrap();
    assert!(error.to_string().contains("default language"));
}

#[test]
fn lists_ids_and_variables_for_parity_checks() {
    let catalog = catalog();
    assert_eq!(catalog.ids(&Locale::EN), ["greeting", "items", "only-en"]);
    assert_eq!(catalog.variables(&Locale::EN, "greeting"), ["name"]);
    assert_eq!(catalog.variables(&Locale::EN, "items"), ["count"]);
    assert!(catalog.variables(&Locale::EN, "only-en").is_empty());
}

#[test]
fn length_limits_agree_with_their_number() {
    let catalog = Catalog::embedded().unwrap();
    let say = |id, name, value: i64| catalog.text(&Message::new(id).arg(name, value));

    assert_eq!(
        say("validation-password-too-short", "min", 1),
        "must be at least 1 character"
    );
    assert_eq!(
        say("note-title-too-long", "max", 200),
        "must be at most 200 characters"
    );
}
