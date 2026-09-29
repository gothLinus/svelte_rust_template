import type { ApiError, ErrorListener } from '$lib/api';

/** A page whose `load` keeps failing must not refresh the session in a loop. */
const REFRESH_INTERVAL_MS = 10_000;

export interface ApiErrorEffects {
	signedIn: () => boolean;
	refreshSession: () => Promise<void>;
	now?: () => number;
}

/**
 * How the app reacts to API errors that concern the whole session rather than one form:
 * the one switch on `error.code` for them. The root layout installs it with
 * `setErrorListener`; 401s go to the unauthenticated handler instead. Forms still get
 * every error and show it.
 */
export function handleApiError(effects: ApiErrorEffects): ErrorListener {
	const now = effects.now ?? Date.now;
	let refreshedAt = -Infinity;

	return (error: ApiError) => {
		switch (error.code) {
			// What the user may do changed since `me` was loaded, e.g. an admin revoked a role
			// or the address was verified in another tab: show the UI for the current state.
			case 'forbidden':
			case 'email_not_verified': {
				if (!effects.signedIn() || now() - refreshedAt < REFRESH_INTERVAL_MS) return;
				refreshedAt = now();
				void effects.refreshSession();
				return;
			}
		}
	};
}
