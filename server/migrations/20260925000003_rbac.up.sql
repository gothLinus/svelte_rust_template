-- Role-based access control. Roles and permissions are reference data, keyed by their
-- stable names rather than UUIDs, so migrations can seed and reference them directly.
--
-- Permissions must match `domain::rbac::Permission` exactly; a test compares the two.
-- To add one: add the enum variant and a migration that inserts it here and grants it.

create table roles (
    name text primary key check (name ~ '^[a-z][a-z0-9_-]{0,49}$'),
    description text not null default '',
    created_at timestamptz not null default now(),
    updated_at timestamptz not null default now()
);

create trigger roles_set_updated_at before update on roles
    for each row execute function set_updated_at();

create table permissions (
    name text primary key check (name ~ '^[a-z_]+:[a-z_]+$'),
    description text not null default '',
    created_at timestamptz not null default now()
);

create table role_permissions (
    role text not null references roles (name) on update cascade on delete cascade,
    permission text not null references permissions (name) on update cascade on delete cascade,
    created_at timestamptz not null default now(),
    primary key (role, permission)
);

create index role_permissions_permission_idx on role_permissions (permission);

create table user_roles (
    user_id uuid not null references users (id) on delete cascade,
    role text not null references roles (name) on update cascade on delete cascade,
    created_at timestamptz not null default now(),
    primary key (user_id, role)
);

create index user_roles_role_idx on user_roles (role);

insert into roles (name, description) values
    ('admin', 'Full access, including user management'),
    ('user', 'Default role for every new account');

insert into permissions (name, description) values
    ('notes:read', 'Read your own notes'),
    ('notes:write', 'Create, edit and delete your own notes'),
    ('notes:manage', 'Read, edit and delete anyone''s notes'),
    ('users:read', 'List and view user accounts'),
    ('users:manage', 'Assign roles, disable and enable accounts');

insert into role_permissions (role, permission)
select 'admin', name from permissions;

insert into role_permissions (role, permission) values
    ('user', 'notes:read'),
    ('user', 'notes:write');
