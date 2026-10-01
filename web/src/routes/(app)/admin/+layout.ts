import { requireAnyPermission } from '$lib/auth';
import { Permission } from '$lib/types/api';
import type { LayoutLoad } from './$types';

export const load: LayoutLoad = async ({ parent }) => {
	const { me } = await parent();
	requireAnyPermission(me, Permission.USERS_READ, Permission.AUDIT_READ);
};
