delete from webauthn_challenges where purpose = 'reauthentication';
alter table webauthn_challenges drop constraint webauthn_challenges_purpose_check;
alter table webauthn_challenges
    add constraint webauthn_challenges_purpose_check
        check (purpose in ('registration', 'authentication', 'second_factor'));

delete from one_time_codes where purpose = 'reauthentication';
alter table one_time_codes drop constraint one_time_codes_purpose_check;
alter table one_time_codes
    add constraint one_time_codes_purpose_check
        check (purpose in ('login', 'phone_verification'));

delete from user_tokens where purpose = 'email_change_cancel';
alter table user_tokens
    drop constraint user_tokens_purpose_check,
    drop constraint user_tokens_new_email_check;
alter table user_tokens
    add constraint user_tokens_purpose_check
        check (purpose in ('email_verification', 'password_reset', 'email_change', 'magic_link')),
    add constraint user_tokens_new_email_check
        check ((purpose = 'email_change') = (new_email is not null)
               and char_length(new_email) between 3 and 254);

alter table sessions drop column reauthenticated_at;
