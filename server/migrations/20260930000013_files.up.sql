-- Files: what users uploaded. The rows hold what the app shows (name, type, size); the
-- contents live in the object store under `object_key`.
create table files (
    id uuid primary key,
    owner_id uuid not null references users (id) on delete cascade,
    name text not null check (char_length(name) between 1 and 255),
    content_type text not null check (char_length(content_type) between 3 and 255),
    size bigint not null check (size between 0 and 26214400),
    object_key text not null unique,
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

-- Serves "my files, newest first" including keyset pagination (`id < $cursor`), and the
-- sum of a user's sizes that the quota checks.
create index files_owner_id_id_idx on files (owner_id, id desc) include (size);

create trigger files_set_updated_at before update on files
    for each row execute function set_updated_at();

-- Objects waiting to be removed from the object store. The store is not part of a
-- database transaction, so its objects are deleted after the rows that name them, by
-- the maintenance job (api/src/jobs.rs), and retried until the store confirms.
--
-- `due_at` null means as soon as possible: a row that named the object is gone. An
-- upload queues its key with a `due_at` in the future before the first byte is stored,
-- and takes it off when its row is committed, so an upload that fails or never finishes
-- leaves nothing behind. A failed deletion moves `due_at` forward.
create table object_deletions (
    object_key text primary key,
    due_at timestamptz,
    created_at timestamptz not null default now()
);

create index object_deletions_due_at_idx on object_deletions (due_at nulls first);

-- Every way a file row goes away queues its object: deleting the file, deleting the
-- account (the cascade above), the retention of unverified accounts, a manual `delete`.
-- Once per statement, so deleting an account with thousands of files is one insert.
create function queue_deleted_file_objects() returns trigger
language plpgsql as $$
begin
    insert into object_deletions (object_key, due_at)
    select object_key, null from deleted_files
    on conflict (object_key) do update set due_at = null;
    return null;
end;
$$;

create trigger files_queue_object_deletion after delete on files
    referencing old table as deleted_files
    for each statement execute function queue_deleted_file_objects();

-- Its permissions, seeded here rather than in the RBAC migration, so the feature brings
-- them along. Keep them in step with `for_each_permission!` and DEFAULT_USER_PERMISSIONS.
insert into permissions (name, description) values
    ('files:read', 'Read and download your own files'),
    ('files:write', 'Upload, rename and delete your own files'),
    ('files:manage', 'Read, rename and delete anyone''s files');

insert into role_permissions (role, permission) values
    ('admin', 'files:read'),
    ('admin', 'files:write'),
    ('admin', 'files:manage'),
    ('user', 'files:read'),
    ('user', 'files:write');
