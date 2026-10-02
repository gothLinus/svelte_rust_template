import {
	AuditEventPageSchema,
	RoleListSchema,
	SessionListSchema,
	UserPageSchema,
	UserSchema
} from '$lib/types/api';
import { type ApiClient, message, segment } from './client';

export interface ListUsersQuery {
	search?: string;
	limit?: number;
	after?: string;
}

export interface ListAuditQuery {
	user?: string;
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
		enable: (id: string) => client.post(`/admin/users/${segment(id)}/enable`, { response: user }),
		sessions: async (id: string) =>
			(
				await client.get(`/admin/users/${segment(id)}/sessions`, {
					response: message(SessionListSchema)
				})
			).sessions,
		revokeSession: (id: string, session: string) =>
			client.delete(`/admin/users/${segment(id)}/sessions/${segment(session)}`),
		/** Ends every session and pending sign-in link; the account stays enabled. */
		signOut: (id: string) => client.delete(`/admin/users/${segment(id)}/sessions`),
		audit: (query: ListAuditQuery = {}) =>
			client.get('/admin/audit', { query: { ...query }, response: message(AuditEventPageSchema) })
	};
}
