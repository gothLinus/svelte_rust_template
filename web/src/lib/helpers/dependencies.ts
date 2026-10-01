/**
 * `depends()` keys of the `load` functions that list data. After changing that data,
 * `invalidate(KEY)` re-runs the matching `load`. The session key is `SESSION` in `$lib/auth`.
 */
export const NOTES = 'app:notes';
export const SESSIONS = 'app:sessions';
export const ADMIN_USERS = 'app:admin-users';
export const AUDIT = 'app:audit';
export const SECURITY = 'app:security';
