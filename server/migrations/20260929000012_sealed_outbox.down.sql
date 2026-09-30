delete from outbox;

drop index outbox_expires_at_idx;
drop index outbox_user_id_idx;

alter table outbox
    drop column user_id,
    drop column expires_at,
    drop column payload,
    add column payload jsonb not null;
