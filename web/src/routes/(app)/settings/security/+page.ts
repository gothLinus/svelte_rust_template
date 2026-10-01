import { createApi } from '$lib/api';
import { ACTIVITY_PAGE_SIZE } from '$lib/helpers/audit';
import { AUDIT, SECURITY, SESSIONS } from '$lib/helpers/dependencies';
import { fromApi } from '$lib/helpers/load';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, parent, depends }) => {
	await parent();
	depends(SESSIONS, SECURITY, AUDIT);
	const api = createApi(fetch);
	const [sessions, security, activity] = await Promise.all([
		fromApi(() => api.me.sessions()),
		fromApi(() => api.me.security()),
		fromApi(() => api.me.activity({ limit: ACTIVITY_PAGE_SIZE }))
	]);
	return { sessions, security, activity };
};
