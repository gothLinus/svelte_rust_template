import { createApi } from '$lib/api';
import { ADMIN_USERS } from '$lib/helpers/dependencies';
import { fromApi } from '$lib/helpers/load';
import type { PageLoad } from './$types';

const PAGE_SIZE = 20;

export const load: PageLoad = async ({ fetch, parent, url, depends }) => {
	await parent();
	depends(ADMIN_USERS);

	const search = url.searchParams.get('search') ?? '';
	const after = url.searchParams.get('after');
	const api = createApi(fetch);
	const [users, roles] = await Promise.all([
		fromApi(() =>
			api.admin.users({ search: search || undefined, after: after ?? undefined, limit: PAGE_SIZE })
		),
		fromApi(() => api.admin.roles())
	]);
	return { users, roles, search, after };
};
