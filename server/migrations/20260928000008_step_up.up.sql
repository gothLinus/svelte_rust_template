-- Step-up re-authentication: sensitive account changes need a recent proof of who is
-- behind the session, not just the session. See application/src/auth/reauth.rs.

-- When the session last proved that: signing in, or re-entering a credential.
alter table sessions add column reauthenticated_at timestamptz;
update sessions set reauthenticated_at = created_at;
alter table sessions alter column reauthenticated_at set not null;

-- The link the old address gets when someone asks to change it: cancels the change (or
-- undoes it, restoring the address the token carries) and signs out every session.
alter table user_tokens
    drop constraint user_tokens_purpose_check,
    drop constraint user_tokens_new_email_check;
alter table user_tokens
    add constraint user_tokens_purpose_check
        check (purpose in (
            'email_verification', 'password_reset', 'email_change', 'magic_link',
            'email_change_cancel'
        )),
    add constraint user_tokens_new_email_check
        check ((purpose in ('email_change', 'email_change_cancel')) = (new_email is not null)
               and char_length(new_email) between 3 and 254);

-- An emailed code that re-authenticates a session.
alter table one_time_codes drop constraint one_time_codes_purpose_check;
alter table one_time_codes
    add constraint one_time_codes_purpose_check
        check (purpose in ('login', 'phone_verification', 'reauthentication'));

-- A passkey ceremony that re-authenticates a session.
alter table webauthn_challenges drop constraint webauthn_challenges_purpose_check;
alter table webauthn_challenges
    add constraint webauthn_challenges_purpose_check
        check (purpose in ('registration', 'authentication', 'second_factor', 'reauthentication'));
