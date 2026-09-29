import { createApi } from '$lib/api';
import { SECURITY, SESSIONS } from '$lib/helpers/dependencies';
import { fromApi } from '$lib/helpers/load';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, parent, depends }) => {
	await parent();
	depends(SESSIONS, SECURITY);
	const api = createApi(fetch);
	const [sessions, security] = await Promise.all([
		fromApi(() => api.me.sessions()),
		fromApi(() => api.me.security())
	]);
	return { sessions, security };
};
