import { redirectIfSignedIn } from '$lib/auth';
import type { LayoutLoad } from './$types';

/**
 * Signing in or up makes no sense with a session: go where the user was headed. The pages
 * below use the sign-in methods the root layout loaded.
 */
export const load: LayoutLoad = async ({ parent, url }) => {
	const { me } = await parent();
	redirectIfSignedIn(me, url);
};
