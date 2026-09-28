import { type SignedIn, createApi, errorMessage } from '$lib/api';
import { SESSION, loadAuthMethods } from '$lib/auth';
import type { LayoutLoad } from './$types';

// A static single-page app: the Rust server (or Vite in development) serves it next to
// `/api`, so it renders in the browser only and `load` functions call the API directly.
export const ssr = false;

/**
 * Who is signed in, and the app's name and sign-in methods (`methods`, for every page
 * below). Never fails: when the server cannot say who this is (it is down, or answers 5xx
 * or 429), the app renders signed out with `sessionError` set, and the layout offers to
 * retry. A failing root `load` would replace every page, public ones included, with
 * SvelteKit's bare fallback page.
 */
export const load: LayoutLoad = async ({ fetch, depends }) => {
	depends(SESSION);
	const [session, methods] = await Promise.all([currentUser(fetch), loadAuthMethods(fetch)]);
	return { ...session, methods };
};

async function currentUser(
	fetchFn: typeof fetch
): Promise<{ me: SignedIn | null; sessionError: string | null }> {
	try {
		return { me: await createApi(fetchFn).me.current(), sessionError: null };
	} catch (error) {
		return { me: null, sessionError: errorMessage(error) };
	}
}
