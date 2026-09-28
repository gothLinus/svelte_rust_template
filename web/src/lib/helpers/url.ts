/**
 * `url`'s path and query with `changes` applied; `null` and empty values remove a parameter.
 * For links between pages of a list, e.g. `withQuery(page.url, { after: cursor })`.
 */
export function withQuery(url: URL, changes: Record<string, string | null | undefined>): string {
	const params = new URLSearchParams(url.search);
	for (const [key, value] of Object.entries(changes)) {
		if (value) params.set(key, value);
		else params.delete(key);
	}
	const search = params.toString();
	return search ? `${url.pathname}?${search}` : url.pathname;
}
