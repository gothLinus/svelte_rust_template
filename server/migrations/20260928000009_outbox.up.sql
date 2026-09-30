-- Mail and text messages waiting for delivery. Rows are written when a request sends
-- something and deleted once the transport accepted it; a message that keeps failing is
-- retried with backoff and dropped (and logged) after its last attempt. See
-- infrastructure/src/outbox.rs.
--
-- The payload holds what is sent, links and codes included, until delivery: a database
-- dump shows the pending ones. They are short-lived, single-use, and stored hashed where
-- they are checked; the outbox only keeps them for the seconds delivery takes.
create table outbox (
    id uuid primary key,
    kind text not null check (kind in ('mail', 'text')),
    payload jsonb not null,
    attempts integer not null default 0 check (attempts >= 0),
    -- When the message may be (re)tried; a worker that claims it moves this forward by
    -- its lease, so a crashed worker's messages come back on their own.
    available_at timestamptz not null default now(),
    last_error text,
    created_at timestamptz not null default now()
);

create index outbox_available_at_idx on outbox (available_at);
