import type { ClientInit, HandleClientError } from '@sveltejs/kit';
import { i18n, t } from '$lib/i18n';
import { captureTokenFromHash } from '$lib/helpers/token';

/**
 * Runs once, before the router starts: takes a token out of the URL (see
 * `$lib/helpers/token`) and picks the language, so the first page and the first API
 * request (`Accept-Language`) are already in it.
 */
export const init: ClientInit = async () => {
	captureTokenFromHash(new URL(location.href), (path) =>
		// The prototype's own method: in development SvelteKit wraps `history.replaceState`
		// to warn about calls behind its back, but its router has not taken over history yet.
		History.prototype.replaceState.call(history, history.state, '', path)
	);
	await i18n.init();
};

/**
 * Errors SvelteKit did not expect: a `load` or component that threw something other than
 * `error(...)`. Report them here (e.g. to an error tracker), and show a message without
 * internals. Errors from `error(...)`, such as a failed API call in `fromApi`, keep theirs.
 */
export const handleError: HandleClientError = ({ error, status }) => {
	if (status === 404) return { message: t('not-found-message') };
	console.error(error);
	return { message: t('app-error-unexpected') };
};
