import { RoleListSchema, UserPageSchema, UserSchema } from '$lib/types/api';
import { type ApiClient, message, segment } from './client';

export interface ListUsersQuery {
	search?: string;
	limit?: number;
	after?: string;
}

const user = message(UserSchema);

export function adminApi(client: ApiClient) {
	return {
		users: (query: ListUsersQuery = {}) =>
			client.get('/admin/users', { query: { ...query }, response: message(UserPageSchema) }),
		user: (id: string) => client.get(`/admin/users/${segment(id)}`, { response: user }),
		roles: async () =>
			(await client.get('/admin/roles', { response: message(RoleListSchema) })).roles,
		grantRole: (id: string, role: string) =>
			client.put(`/admin/users/${segment(id)}/roles/${segment(role)}`, { response: user }),
		revokeRole: (id: string, role: string) =>
			client.delete(`/admin/users/${segment(id)}/roles/${segment(role)}`, { response: user }),
		disable: (id: string) => client.post(`/admin/users/${segment(id)}/disable`, { response: user }),
		enable: (id: string) => client.post(`/admin/users/${segment(id)}/enable`, { response: user })
	};
}
