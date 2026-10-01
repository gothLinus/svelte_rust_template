import { isHttpError, isRedirect } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { API_BASE } from '$lib/api';
import { FALLBACK_APP_NAME, SESSION } from '$lib/auth';
import { load as rootLoad } from '../../src/routes/+layout';
import { load as appLoad } from '../../src/routes/(app)/+layout';
import { load as adminLoad } from '../../src/routes/(app)/admin/+layout';
import { load as auditLoad } from '../../src/routes/(app)/admin/audit/+page';
import { load as usersLoad } from '../../src/routes/(app)/admin/users/+page';
import { load as dashboardLoad } from '../../src/routes/(app)/dashboard/+page';
import { load as notesLoad } from '../../src/routes/(app)/notes/+page';
import { load as settingsLoad } from '../../src/routes/(app)/settings/+page';
import { load as securityLoad } from '../../src/routes/(app)/settings/security/+page';
import { load as guestLoad } from '../../src/routes/(public)/(guest)/+layout';
import {
	AuditEventPageSchema,
	AuthMethodsSchema,
	MeSchema,
	NotePageSchema,
	Permission,
	RoleListSchema,
	SecurityOverviewSchema,
	SessionListSchema,
	UserPageSchema
} from '$lib/types/api';
import { loadEvent, me, methods, mockFetch, note, problem, reply, routeFetch } from '../helpers';

function loaded<T>(data: T | void): T {
	if (!data) throw new Error('the load function returned nothing');
	return data;
}

async function rejection(promise: unknown): Promise<unknown> {
	try {
		await promise;
	} catch (error) {
		return error;
	}
	throw new Error('expected a rejection');
}

describe('root layout', () => {
	it('loads the session and the app name, and depends on the session', async () => {
		const depends = vi.fn();
		const fetchFn = routeFetch({
			'/me': reply(MeSchema, me()),
			'/auth/methods': reply(AuthMethodsSchema, { appName: 'Widgets' })
		});

		const data = await rootLoad(loadEvent({ fetch: fetchFn, depends }));

		expect(data).toEqual({
			me: me(),
			sessionError: null,
			methods: methods({ appName: 'Widgets' })
		});
		expect(depends).toHaveBeenCalledWith(SESSION);
	});

	it('is anonymous without a session', async () => {
		const data = await rootLoad(
			loadEvent({ fetch: mockFetch(problem(401, 'unauthenticated')), depends: vi.fn() })
		);
		expect(data).toEqual({
			me: null,
			sessionError: null,
			methods: methods({ appName: FALLBACK_APP_NAME })
		});
	});

	it('never fails, so every page still renders when the server does not answer', async () => {
		for (const [status, code] of [
			[503, 'unavailable'],
			[429, 'rate_limited']
		] as const) {
			const fetchFn = mockFetch(problem(status, code));
			const data = await rootLoad(loadEvent({ fetch: fetchFn, depends: vi.fn() }));
			expect(data).toMatchObject({ me: null, sessionError: `problem ${code}` });
		}

		const offline = vi.fn<typeof fetch>(async () => {
			throw new TypeError('Failed to fetch');
		});
		const data = await rootLoad(loadEvent({ fetch: offline, depends: vi.fn() }));
		expect(data).toMatchObject({ me: null, sessionError: expect.stringContaining('reach') });
	});
});

describe('(app) layout', () => {
	it('lets signed-in users in', async () => {
		const data = await appLoad(
			loadEvent({ parent: async () => ({ me: me() }), url: new URL('http://app.test/notes') })
		);
		expect(data).toEqual({ me: me() });
	});

	it('sends everyone else to sign in', async () => {
		const error = await rejection(
			appLoad(
				loadEvent({ parent: async () => ({ me: null }), url: new URL('http://app.test/notes') })
			)
		);
		expect(isRedirect(error)).toBe(true);
		expect(error).toMatchObject({ location: '/login?redirectTo=%2Fnotes' });
	});

	it('shows an error, not the login page, when the session could not be checked', async () => {
		const error = await rejection(
			appLoad(
				loadEvent({
					parent: async () => ({ me: null, sessionError: 'The server is down.' }),
					url: new URL('http://app.test/notes')
				})
			)
		);
		expect(isHttpError(error, 503)).toBe(true);
		expect(error).toMatchObject({ body: { message: 'The server is down.' } });
	});
});

describe('admin layout', () => {
	it('needs users:read or audit:read', async () => {
		for (const permission of [Permission.USERS_READ, Permission.AUDIT_READ]) {
			await expect(
				adminLoad(loadEvent({ parent: async () => ({ me: me([permission]) }) }))
			).resolves.toBeUndefined();
		}

		const error = await rejection(adminLoad(loadEvent({ parent: async () => ({ me: me() }) })));
		expect(isHttpError(error, 403)).toBe(true);
	});
});

describe('guest layout', () => {
	it('redirects signed-in users', async () => {
		const error = await rejection(
			guestLoad(
				loadEvent({ parent: async () => ({ me: me() }), url: new URL('http://app.test/login') })
			)
		);
		expect(error).toMatchObject({ status: 307, location: '/dashboard' });

		await expect(
			guestLoad(
				loadEvent({ parent: async () => ({ me: null }), url: new URL('http://app.test/login') })
			)
		).resolves.toBeUndefined();
	});
});

describe('page loads', () => {
	it('notes: reads scope and cursor from the URL', async () => {
		const fetchFn = mockFetch(reply(NotePageSchema, { items: [note()], nextCursor: 'c2' }));
		const depends = vi.fn();

		const data = await notesLoad(
			loadEvent({
				fetch: fetchFn,
				parent: async () => ({ me: me() }),
				depends,
				url: new URL('http://app.test/notes?scope=all&after=c1')
			})
		);

		expect(data).toMatchObject({ scope: 'all', after: 'c1' });
		expect(String(fetchFn.mock.calls[0]?.[0])).toBe(
			`${API_BASE}/notes?scope=all&after=c1&limit=12`
		);
		expect(depends).toHaveBeenCalledWith('app:notes');

		const mine = await notesLoad(
			loadEvent({
				fetch: fetchFn,
				parent: async () => ({ me: me() }),
				depends,
				url: new URL('http://app.test/notes?scope=weird')
			})
		);
		expect(mine).toMatchObject({ scope: 'mine', after: null });
	});

	it('notes: API errors become error pages', async () => {
		const error = await rejection(
			notesLoad(
				loadEvent({
					fetch: mockFetch(problem(403, 'forbidden')),
					parent: async () => ({ me: me() }),
					depends: vi.fn(),
					url: new URL('http://app.test/notes?scope=all')
				})
			)
		);
		expect(isHttpError(error, 403)).toBe(true);
	});

	it('dashboard: sessions and recent notes', async () => {
		const fetchFn = mockFetch(reply(SessionListSchema), reply(NotePageSchema, { items: [note()] }));
		const data = await dashboardLoad(
			loadEvent({ fetch: fetchFn, parent: async () => ({ me: me() }), depends: vi.fn() })
		);
		expect(data).toEqual({ sessions: [], recentNotes: [note()] });

		const noNotes = loaded(
			await dashboardLoad(
				loadEvent({
					fetch: mockFetch(reply(SessionListSchema)),
					parent: async () => ({ me: me([]) }),
					depends: vi.fn()
				})
			)
		);
		expect(noNotes.recentNotes).toEqual([]);
	});

	it('security: sessions and the overview', async () => {
		const depends = vi.fn();
		const data = await securityLoad(
			loadEvent({
				fetch: routeFetch({
					'/me/sessions': reply(SessionListSchema, { sessions: [{ id: 's1' }] }),
					'/me/security': reply(SecurityOverviewSchema, { hasPassword: true }),
					'/me/activity': reply(AuditEventPageSchema, { items: [{ id: 'e1' }] })
				}),
				parent: async () => ({ me: me() }),
				depends
			})
		);
		expect(data).toMatchObject({
			sessions: [{ id: 's1' }],
			security: { hasPassword: true, passkeys: [] },
			activity: { items: [{ id: 'e1' }] }
		});
		expect(depends).toHaveBeenCalledWith('app:sessions', 'app:security', 'app:audit');
	});

	it('admin users: search, cursor and roles', async () => {
		const fetchFn = mockFetch(
			reply(UserPageSchema),
			reply(RoleListSchema, { roles: [{ name: 'admin' }] })
		);
		const data = loaded(
			await usersLoad(
				loadEvent({
					fetch: fetchFn,
					parent: async () => ({ me: me([Permission.USERS_READ]) }),
					depends: vi.fn(),
					url: new URL('http://app.test/admin/users?search=ali&after=c1')
				})
			)
		);

		expect(data.search).toBe('ali');
		expect(data.after).toBe('c1');
		expect(data.roles).toHaveLength(1);
		expect(String(fetchFn.mock.calls[0]?.[0])).toBe(
			`${API_BASE}/admin/users?search=ali&after=c1&limit=20`
		);
	});

	it('admin users: needs users:read even though the area admits audit:read', async () => {
		const error = await rejection(
			usersLoad(
				loadEvent({
					fetch: mockFetch(reply(UserPageSchema)),
					parent: async () => ({ me: me([Permission.AUDIT_READ]) }),
					depends: vi.fn(),
					url: new URL('http://app.test/admin/users')
				})
			)
		);
		expect(isHttpError(error, 403)).toBe(true);
	});

	it('audit log: needs audit:read and reads the user filter and cursor', async () => {
		const fetchFn = mockFetch(reply(AuditEventPageSchema, { nextCursor: 'c2' }));
		const depends = vi.fn();
		const data = loaded(
			await auditLoad(
				loadEvent({
					fetch: fetchFn,
					parent: async () => ({ me: me([Permission.AUDIT_READ]) }),
					depends,
					url: new URL('http://app.test/admin/audit?user=u1&after=c1')
				})
			)
		);

		expect(data).toMatchObject({ user: 'u1', after: 'c1', events: { nextCursor: 'c2' } });
		expect(String(fetchFn.mock.calls[0]?.[0])).toBe(
			`${API_BASE}/admin/audit?user=u1&after=c1&limit=25`
		);
		expect(depends).toHaveBeenCalledWith('app:audit');

		const error = await rejection(
			auditLoad(
				loadEvent({
					fetch: fetchFn,
					parent: async () => ({ me: me([Permission.USERS_READ]) }),
					depends,
					url: new URL('http://app.test/admin/audit')
				})
			)
		);
		expect(isHttpError(error, 403)).toBe(true);
	});

	it('settings redirects to the profile', () => {
		let error: unknown;
		try {
			void settingsLoad(loadEvent({}));
		} catch (e) {
			error = e;
		}
		expect(error).toMatchObject({ status: 307, location: '/settings/profile' });
	});
});
