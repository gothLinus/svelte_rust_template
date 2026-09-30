-- Messages in the outbox hold working links and codes until delivery. From now on the
-- payload is sealed (AES-256-GCM under SECRET_KEY, bound to the row id and kind), so a
-- database dump, a replica or the WAL shows ciphertext; see infrastructure/src/outbox.rs.
--
-- Rows queued before this migration are plain JSON and cannot be sealed in SQL, so they
-- are dropped: every message the app sends can be asked for again.
delete from outbox;

alter table outbox
    drop column payload,
    add column payload bytea not null,
    -- Past this, delivering is pointless (the link or code in it expired): the row is
    -- dropped instead of retried.
    add column expires_at timestamptz not null,
    -- The recipient's account, when the address belongs to one: deleting the account
    -- deletes its undelivered mail.
    add column user_id uuid references users (id) on delete cascade;

create index outbox_user_id_idx on outbox (user_id) where user_id is not null;
create index outbox_expires_at_idx on outbox (expires_at);
