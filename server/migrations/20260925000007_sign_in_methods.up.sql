-- Sign-in methods beyond email or username and password: phone numbers, emailed and texted
-- codes, magic links, social accounts, passkeys and two-step sign-in.

-- Accounts created through a social provider or a passkey may have no password.
alter table users alter column password_hash drop not null;

alter table users
    -- E.164.
    add column phone text check (phone ~ '^\+[1-9][0-9]{6,14}$'),
    add column phone_verified_at timestamptz;

create unique index users_phone_key on users (phone);

alter table user_tokens drop constraint user_tokens_purpose_check;
alter table user_tokens
    add constraint user_tokens_purpose_check
        check (purpose in ('email_verification', 'password_reset', 'email_change', 'magic_link')),
    -- The address an `email_change` token confirms.
    add column new_email text,
    add constraint user_tokens_new_email_check
        check ((purpose = 'email_change') = (new_email is not null)
               and char_length(new_email) between 3 and 254);

-- Six-digit codes sent by email or text. See domain/src/one_time_code.
create table one_time_codes (
    user_id uuid not null references users (id) on delete cascade,
    purpose text not null check (purpose in ('login', 'phone_verification')),
    channel text not null check (channel in ('email', 'sms', 'whatsapp')),
    code_hash bytea not null check (octet_length(code_hash) = 32),
    -- The phone number a `phone_verification` code confirms.
    target text,
    attempts integer not null default 0 check (attempts >= 0),
    expires_at timestamptz not null,
    created_at timestamptz not null default now(),
    primary key (user_id, purpose)
);

create index one_time_codes_expires_at_idx on one_time_codes (expires_at);

-- Accounts at social sign-in providers linked to users.
create table external_identities (
    id uuid primary key,
    user_id uuid not null references users (id) on delete cascade,
    provider text not null check (provider ~ '^[a-z][a-z0-9_-]{0,49}$'),
    subject text not null check (char_length(subject) between 1 and 255),
    email text,
    created_at timestamptz not null default now(),
    last_used_at timestamptz not null default now(),
    constraint external_identities_provider_subject_key unique (provider, subject),
    constraint external_identities_user_id_provider_key unique (user_id, provider)
);

-- Sign-ins at a provider in progress, keyed by the SHA-256 of their `state` parameter.
create table oauth_flows (
    state_hash bytea primary key check (octet_length(state_hash) = 32),
    provider text not null,
    pkce_verifier text not null,
    nonce text not null,
    -- Set when a signed-in user links an account.
    link_user_id uuid references users (id) on delete cascade,
    redirect_to text not null,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index oauth_flows_expires_at_idx on oauth_flows (expires_at);

-- WebAuthn credentials. See domain/src/passkey.
create table passkeys (
    id uuid primary key,
    user_id uuid not null references users (id) on delete cascade,
    credential_id bytea not null check (octet_length(credential_id) between 1 and 1023),
    -- DER SubjectPublicKeyInfo.
    public_key bytea not null,
    -- COSE algorithm identifier.
    algorithm integer not null check (algorithm in (-7, -8, -257)),
    sign_count bigint not null default 0 check (sign_count between 0 and 4294967295),
    transports text[] not null default '{}',
    name text not null check (char_length(name) between 1 and 100),
    created_at timestamptz not null default now(),
    last_used_at timestamptz,
    constraint passkeys_credential_id_key unique (credential_id)
);

create index passkeys_user_id_idx on passkeys (user_id, created_at);

create table webauthn_challenges (
    id uuid primary key,
    challenge bytea not null check (octet_length(challenge) between 16 and 64),
    purpose text not null check (purpose in ('registration', 'authentication', 'second_factor')),
    user_id uuid references users (id) on delete cascade,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index webauthn_challenges_expires_at_idx on webauthn_challenges (expires_at);

-- Authenticator apps. The secret is encrypted with SECRET_KEY (AES-256-GCM).
create table totp_credentials (
    user_id uuid primary key references users (id) on delete cascade,
    sealed_secret bytea not null,
    confirmed_at timestamptz,
    last_used_step bigint,
    created_at timestamptz not null default now()
);

create table recovery_codes (
    user_id uuid not null references users (id) on delete cascade,
    code_hash bytea not null check (octet_length(code_hash) = 32),
    created_at timestamptz not null default now(),
    primary key (user_id, code_hash)
);

-- Users between the first and the second sign-in step.
create table mfa_challenges (
    token_hash bytea primary key check (octet_length(token_hash) = 32),
    user_id uuid not null references users (id) on delete cascade,
    attempts integer not null default 0 check (attempts >= 0),
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);

create index mfa_challenges_expires_at_idx on mfa_challenges (expires_at);
