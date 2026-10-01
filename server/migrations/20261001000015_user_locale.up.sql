-- The language mail and texts to the user are written in, as a BCP 47 tag such as `de` or
-- `pt-BR` (`domain::i18n::Locale`). Null leaves it to the server's default.
alter table users
    add column locale text
        check (char_length(locale) between 1 and 35 and locale ~ '^[A-Za-z][A-Za-z0-9-]*$');
