import { error } from '@sveltejs/kit';
import { ApiError } from '$lib/api';

/**
 * Runs an API call inside a `load` function and turns a failure into SvelteKit's error page
 * with the same status and message.
 *
 * SvelteKit runs a page's `load` in parallel with its layouts'. Under `(app)`, `await
 * parent()` first, so the layouts' guards run before the request goes out.
 */
export async function fromApi<T>(call: () => Promise<T>): Promise<T> {
	try {
		return await call();
	} catch (err) {
		if (err instanceof ApiError) error(err.status || 503, err.message);
		throw err;
	}
}
