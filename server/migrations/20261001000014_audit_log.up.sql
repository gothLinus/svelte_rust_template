-- The audit log: security events per account, appended in the transaction of the change they
-- record. See domain::audit.
create table audit_events (
    -- UUIDv7, so the primary key orders events by time for keyset pagination.
    id uuid primary key,
    -- The account the event is about; its history goes with it.
    user_id uuid not null references users (id) on delete cascade,
    -- Who caused it, when someone was signed in: the user or an administrator. Not a foreign
    -- key: the record of who acted outlives their account.
    actor_id uuid,
    -- `domain::audit::AuditAction`.
    action text not null check (action ~ '^[a-z_]{1,64}$'),
    detail text check (char_length(detail) <= 200),
    ip inet,
    user_agent text check (char_length(user_agent) <= 512),
    occurred_at timestamptz not null
);

-- "This account's events, newest first", with keyset pagination (`id < $cursor`).
create index audit_events_user_id_id_idx on audit_events (user_id, id desc);
-- For the retention cleanup.
create index audit_events_occurred_at_idx on audit_events (occurred_at);

insert into permissions (name, description) values
    ('audit:read', 'View the audit log of every account');

insert into role_permissions (role, permission) values
    ('admin', 'audit:read');
