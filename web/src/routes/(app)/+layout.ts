import { error } from '@sveltejs/kit';
import { requireUser } from '$lib/auth';
import type { LayoutLoad } from './$types';

/**
 * Every page in this group needs a session: without one, go to the login page and come
 * back afterwards. For the user's convenience only; the API checks every request itself.
 */
export const load: LayoutLoad = async ({ parent, url }) => {
	const { me, sessionError } = await parent();
	// The server could not say who this is; sending a signed-in user to the login page
	// would be wrong. The root error page offers to try again.
	if (!me && sessionError) error(503, sessionError);
	return { me: requireUser(me, url) };
};
