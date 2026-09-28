-- Indexes for the foreign keys that had none. Deleting a user cascades into these tables,
-- and a password reset or signing out everywhere deletes the user's MFA challenges by
-- `user_id`: without an index each of those scans the whole table.
create index mfa_challenges_user_id_idx on mfa_challenges (user_id);
create index webauthn_challenges_user_id_idx on webauthn_challenges (user_id);
create index oauth_flows_link_user_id_idx on oauth_flows (link_user_id);
