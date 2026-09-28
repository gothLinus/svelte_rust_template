import { createApi } from '$lib/api';
import { hasPermission } from '$lib/auth';
import { Permission } from '$lib/types/api';
import { NOTES, SESSIONS } from '$lib/helpers/dependencies';
import { fromApi } from '$lib/helpers/load';
import type { PageLoad } from './$types';

export const load: PageLoad = async ({ fetch, parent, depends }) => {
	const { me } = await parent();
	depends(SESSIONS, NOTES);
	const api = createApi(fetch);

	const [sessions, notes] = await Promise.all([
		fromApi(() => api.me.sessions()),
		hasPermission(me, Permission.NOTES_READ)
			? fromApi(() => api.notes.list({ limit: 3 }))
			: Promise.resolve(null)
	]);
	return { sessions, recentNotes: notes?.items ?? [] };
};
