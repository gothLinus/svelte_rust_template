drop table mfa_challenges;
drop table recovery_codes;
drop table totp_credentials;
drop table webauthn_challenges;
drop table passkeys;
drop table oauth_flows;
drop table external_identities;
drop table one_time_codes;

delete from user_tokens where purpose in ('email_change', 'magic_link');
alter table user_tokens
    drop constraint user_tokens_new_email_check,
    drop column new_email,
    drop constraint user_tokens_purpose_check;
alter table user_tokens
    add constraint user_tokens_purpose_check
        check (purpose in ('email_verification', 'password_reset'));

drop index users_phone_key;
alter table users
    drop column phone_verified_at,
    drop column phone;

-- Accounts without a password cannot be represented any more.
delete from users where password_hash is null;
alter table users alter column password_hash set not null;
