-- The example resource. Copy this migration when adding one.
create table notes (
    id uuid primary key,
    owner_id uuid not null references users (id) on delete cascade,
    title text not null check (char_length(title) between 1 and 200),
    body text not null default '' check (char_length(body) <= 10000),
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Serves "my notes, newest first" including keyset pagination (`id < $cursor`).
create index notes_owner_id_id_idx on notes (owner_id, id desc);

create trigger notes_set_updated_at before update on notes
    for each row execute function set_updated_at();
