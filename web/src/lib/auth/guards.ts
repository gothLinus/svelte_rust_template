import { error, redirect } from '@sveltejs/kit';
import type { SignedIn } from '$lib/api';
import { t } from '$lib/i18n';
import type { Permission } from '$lib/types/api';
import { hasPermission } from './permissions';

/**
 * Route guards for `+layout.ts` `load` functions. They only decide what to show: the API
 * rejects anything the user may not do, whatever the UI lets through.
 */

export const SESSION = 'app:session';

export const HOME = '/dashboard';

const MAX_REDIRECT_LENGTH = 512;
// Browsers drop tabs and newlines inside URLs and read `\` as `/`, so `/\t/evil.example`
// would become `//evil.example`: another host.
// eslint-disable-next-line no-control-regex
const UNSAFE_REDIRECT = /[\u0000-\u001f\u007f\\]/;
// A base that cannot collide with a real origin, to tell paths on this site from URLs.
const SAME_SITE = 'http://same-site.invalid';

export function loginUrl(url: URL): string {
	const target = url.pathname + url.search;
	return target === '/' || target.startsWith('/login')
		? '/login'
		: `/login?redirectTo=${encodeURIComponent(target)}`;
}

/**
 * `target` if it is a page of this app, otherwise `fallback`. Never follow `redirectTo`
 * blindly: `//evil.example`, `/\evil.example` and `/%09/evil.example` are URLs to another
 * host, and `/api/...` is not a page. Mirrors `safe_redirect` in the server's OAuth service.
 */
export function safeRedirect(target: string | null | undefined, fallback = HOME): string {
	if (!target || target.length > MAX_REDIRECT_LENGTH || UNSAFE_REDIRECT.test(target)) {
		return fallback;
	}
	if (!target.startsWith('/') || target.startsWith('//')) return fallback;
	const url = new URL(target, SAME_SITE);
	if (url.origin !== SAME_SITE || /^\/api(?:\/|$)/.test(url.pathname)) return fallback;
	return url.pathname + url.search + url.hash;
}

/** Signed-in users only: anyone else goes to the login page and comes back afterwards. */
export function requireUser(me: SignedIn | null, url: URL): SignedIn {
	if (!me) redirect(307, loginUrl(url));
	return me;
}

export function requirePermission(me: SignedIn, permission: Permission): void {
	if (!hasPermission(me, permission)) {
		error(403, t('error-forbidden'));
	}
}

export function redirectIfSignedIn(me: SignedIn | null, url: URL): void {
	if (me) redirect(307, safeRedirect(url.searchParams.get('redirectTo')));
}
