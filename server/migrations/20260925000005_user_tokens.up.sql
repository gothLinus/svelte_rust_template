-- Single-use tokens sent by email (address verification, password reset).
create table user_tokens (
    -- SHA-256 of the token in the link; the token itself is never stored.
    token_hash bytea primary key check (octet_length(token_hash) = 32),
    user_id uuid not null references users (id) on delete cascade,
    purpose text not null check (purpose in ('email_verification', 'password_reset')),
    expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    -- One live token per user and purpose: a new link invalidates the previous one.
    constraint user_tokens_user_id_purpose_key unique (user_id, purpose)
);

create index user_tokens_expires_at_idx on user_tokens (expires_at);
