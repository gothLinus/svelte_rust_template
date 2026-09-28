-- Rate limit buckets shared by every instance (RATE_LIMIT_STORE=postgres). See
-- infrastructure/src/rate_limit.rs.
--
-- Unlogged: writes skip the WAL, which matters for a table written on every rate limited
-- request. A database crash empties it, which only resets the limits.
create unlogged table rate_limits (
    -- Which limit, e.g. `login_ip`.
    bucket text not null check (char_length(bucket) <= 64),
    -- Who within it: an IP network, an account, a user.
    key text not null check (char_length(key) <= 512),
    -- When the bucket is full again (the GCRA's theoretical arrival time).
    tat timestamptz not null,
    primary key (bucket, key)
);

create index rate_limits_tat_idx on rate_limits (tat);
