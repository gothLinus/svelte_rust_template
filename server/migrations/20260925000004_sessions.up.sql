-- Browser sessions.
create table sessions (
    id uuid primary key,
    user_id uuid not null references users (id) on delete cascade,
    -- SHA-256 of the cookie token, so a database leak does not hand out live sessions.
    token_hash bytea not null check (octet_length(token_hash) = 32),
    ip inet,
    user_agent text check (char_length(user_agent) <= 512),
    -- Set when the user's roles change; the next request gets a new token.
    rotation_pending boolean not null default false,
    created_at timestamptz not null default now(),
    -- Drives the idle timeout. Written at most once a minute per session.
    last_seen_at timestamptz not null,
    -- The absolute expiry, fixed at sign-in.
    expires_at timestamptz not null,
    updated_at timestamptz not null default now()
);

create unique index sessions_token_hash_key on sessions (token_hash);
create index sessions_user_id_idx on sessions (user_id, last_seen_at desc);
-- For the cleanup task.
create index sessions_expires_at_idx on sessions (expires_at);
create index sessions_last_seen_at_idx on sessions (last_seen_at);

create trigger sessions_set_updated_at before update on sessions
    for each row execute function set_updated_at();
