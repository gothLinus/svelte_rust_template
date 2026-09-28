/**
 * The token of an emailed link (`/reset-password#token=...`), read from the URL fragment.
 * Fragments never reach a server, which keeps tokens out of logs and `Referer` headers.
 *
 * The client `init` hook captures it once, before SvelteKit's router starts, and removes
 * the fragment from the address bar and history so it does not linger. Doing that from a
 * page's `onMount` races the router, whose `replaceState` refuses to run until it has
 * started, and on a fresh page load (every emailed link) it has not.
 */
let captured: { path: string; token: string } | null = null;

/**
 * Takes a `token` out of the fragment of `url`, the page being loaded. Other fragments,
 * such as `#section` anchors, are left alone. `replace` swaps the current history entry
 * for the given path.
 */
export function captureTokenFromHash(url: URL, replace: (path: string) => void): void {
	const token = new URLSearchParams(url.hash.slice(1)).get('token');
	if (!token) return;
	captured = { path: url.pathname, token };
	replace(url.pathname + url.search);
}

export function takeToken(path: string): string | null {
	const token = captured?.path === path ? captured.token : null;
	captured = null;
	return token;
}
