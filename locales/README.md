# Locales

User-facing text lives here as [Fluent](https://projectfluent.org) files, one directory
per language:

```
locales/
  en/
    server/*.ftl    read by the API (embedded by crates/i18n at build time)
    web/*.ftl       read by the SvelteKit app
```

`en` is the source language and the fallback. To add a language, copy `en/` to a new
directory named by its BCP 47 tag and translate it; no code changes. `cargo test -p i18n`
and `bun run test catalog` fail on a missing or extra message and on mismatched
placeholders.

## Conventions

- Ids are lowercase and dash-separated, prefixed by area: `login-title`,
  `validation-too-long`. They are unique per side (server, web).
- Placeholders are Fluent variables (`{ $max }`) passed by name.
- Plurals use selectors, not conditionals in code:
  ```ftl
  notes-count = { $count ->
      [one] { $count } note
     *[other] { $count } notes
  }
  ```
- Messages are whole sentences. Do not assemble them from fragments.
- Files are grouped by area. The example resource keeps its messages in `notes.ftl`,
  which `just new-resource` copies.
- API error codes (`too_long`, `email_taken`) are never translated, only the message.
- `#` comments tell translators where a text appears.

## Usage

Rust: services build a `domain::i18n::Message` and the API renders it for the request's
`Accept-Language`.

```rust
ValidationError::new("too_long", Message::new("validation-too-long").arg("max", 100))
```

Web: `t('login-title')` or `t('notes-count', { count })` from `$lib/i18n`.

Tests read expected text from the catalog (`t('login-title')`) instead of repeating it.
