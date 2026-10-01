import { createApi } from '$lib/api';
import { requirePermission } from '$lib/auth';
import { AUDIT } from '$lib/helpers/dependencies';
import { fromApi } from '$lib/helpers/load';
import { Permission } from '$lib/types/api';
import type { PageLoad } from './$types';

const PAGE_SIZE = 25;

export const load: PageLoad = async ({ fetch, parent, url, depends }) => {
	const { me } = await parent();
	requirePermission(me, Permission.AUDIT_READ);
	depends(AUDIT);

	const user = url.searchParams.get('user');
	const after = url.searchParams.get('after');
	const api = createApi(fetch);
	const events = await fromApi(() =>
		api.admin.audit({ user: user ?? undefined, after: after ?? undefined, limit: PAGE_SIZE })
	);
	return { events, user, after };
};
