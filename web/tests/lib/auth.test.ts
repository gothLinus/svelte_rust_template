import { isHttpError, isRedirect } from '@sveltejs/kit';
import { describe, expect, it, vi } from 'vitest';
import { ApiError } from '$lib/api';
import { t } from '$lib/i18n';
import {
	HOME,
	ReauthPrompt,
	SessionState,
	handleApiError,
	hasAnyPermission,
	hasPermission,
	loginUrl,
	notePolicy,
	ownerOr,
	redirectIfSignedIn,
	requirePermission,
	requireUser,
	safeRedirect
} from '$lib/auth';
import { Permission } from '$lib/types/api';
import { OTHER_ID, USER_ID, me, note } from '../helpers';

function thrown(fn: () => unknown): unknown {
	try {
		fn();
	} catch (error) {
		return error;
	}
	throw new Error('expected a throw');
}

describe('permissions', () => {
	it('checks single and any permissions', () => {
		const user = me([Permission.NOTES_READ]);
		expect(hasPermission(user, Permission.NOTES_READ)).toBe(true);
		expect(hasPermission(user, Permission.NOTES_WRITE)).toBe(false);
		expect(hasPermission(null, Permission.NOTES_READ)).toBe(false);
		expect(hasAnyPermission(user, Permission.USERS_READ, Permission.NOTES_READ)).toBe(true);
		expect(hasAnyPermission(user, Permission.USERS_READ)).toBe(false);
	});

	it('applies ownership', () => {
		const own = note(USER_ID);
		const foreign = note(OTHER_ID);
		const writer = me([Permission.NOTES_WRITE]);
		const manager = me([Permission.NOTES_MANAGE]);

		expect(ownerOr(writer, own, Permission.NOTES_WRITE, Permission.NOTES_MANAGE)).toBe(true);
		expect(ownerOr(writer, foreign, Permission.NOTES_WRITE, Permission.NOTES_MANAGE)).toBe(false);
		expect(ownerOr(manager, foreign, Permission.NOTES_WRITE, Permission.NOTES_MANAGE)).toBe(true);
		expect(ownerOr(null, own, Permission.NOTES_WRITE, Permission.NOTES_MANAGE)).toBe(false);
	});

	it('mirrors the note policy', () => {
		const user = me([Permission.NOTES_READ, Permission.NOTES_WRITE]);
		const reader = me([Permission.NOTES_READ]);
		const manager = me([Permission.NOTES_MANAGE]);
		const own = note(USER_ID);
		const foreign = note(OTHER_ID);

		expect(notePolicy(user, 'read')).toBe(true);
		expect(notePolicy(user, 'create')).toBe(true);
		expect(notePolicy(user, 'read', own)).toBe(true);
		expect(notePolicy(user, 'update', own)).toBe(true);
		expect(notePolicy(user, 'delete', foreign)).toBe(false);
		expect(notePolicy(user, 'read', foreign)).toBe(false);

		expect(notePolicy(reader, 'update', own)).toBe(false);
		expect(notePolicy(reader, 'create')).toBe(false);

		expect(notePolicy(manager, 'read')).toBe(true);
		expect(notePolicy(manager, 'delete', foreign)).toBe(true);
		expect(notePolicy(manager, 'create')).toBe(false);
		expect(notePolicy(manager, 'update')).toBe(false);

		expect(notePolicy(null, 'read')).toBe(false);
	});
});

describe('SessionState', () => {
	it('tracks the signed-in user', () => {
		const session = new SessionState();
		expect(session.isAuthenticated).toBe(false);
		expect(session.user).toBeNull();
		expect(session.needsVerification).toBe(false);

		const unverified = me([Permission.NOTES_READ]);
		unverified.user.emailVerified = false;
		session.connect(() => unverified);

		expect(session.isAuthenticated).toBe(true);
		expect(session.user?.email).toBe('alice@example.com');
		expect(session.needsVerification).toBe(true);
		expect(session.has(Permission.NOTES_READ)).toBe(true);
		expect(session.has(Permission.USERS_READ)).toBe(false);
		expect(session.can(notePolicy, 'read', note())).toBe(true);
		expect(session.can(notePolicy, 'update', note())).toBe(false);
		expect(session.can(notePolicy, 'read')).toBe(true);

		session.connect(() => null);
		expect(session.isAuthenticated).toBe(false);
	});
});

describe('guards', () => {
	it('builds login URLs that come back', () => {
		expect(loginUrl(new URL('http://app.test/notes?after=x'))).toBe(
			'/login?redirectTo=%2Fnotes%3Fafter%3Dx'
		);
		expect(loginUrl(new URL('http://app.test/'))).toBe('/login');
		expect(loginUrl(new URL('http://app.test/login?redirectTo=%2Fnotes'))).toBe('/login');
	});

	it('only redirects to pages of this app', () => {
		expect(HOME).toBe('/dashboard');
		expect(safeRedirect('/notes?x=1#top')).toBe('/notes?x=1#top');
		expect(safeRedirect('/apiary')).toBe('/apiary');
		expect(safeRedirect('/a/../notes')).toBe('/notes');
		for (const target of [
			null,
			undefined,
			'',
			'notes',
			'https://evil.example',
			'//evil.example',
			'/\\evil',
			// Browsers drop tabs and newlines, which would leave `//evil.example`.
			'/\t/evil.example',
			'/\n/evil.example',
			'/\r/evil.example',
			'\t//evil.example',
			'/\u0000/evil.example',
			'/\u007f',
			'/api',
			'/api/v1/auth/logout',
			'/notes/../api/v1/me',
			`/${'a'.repeat(512)}`
		]) {
			expect(safeRedirect(target), JSON.stringify(target)).toBe(HOME);
		}
		expect(safeRedirect(`/${'a'.repeat(511)}`)).toBe(`/${'a'.repeat(511)}`);
		expect(safeRedirect(null, '/settings/profile')).toBe('/settings/profile');
	});

	it('requires a user', () => {
		const user = me();
		expect(requireUser(user, new URL('http://app.test/notes'))).toBe(user);

		const redirect = thrown(() => requireUser(null, new URL('http://app.test/notes')));
		expect(isRedirect(redirect)).toBe(true);
		expect(redirect).toMatchObject({ status: 307, location: '/login?redirectTo=%2Fnotes' });
	});

	it('requires a permission', () => {
		expect(() =>
			requirePermission(me([Permission.USERS_READ]), Permission.USERS_READ)
		).not.toThrow();
		const error = thrown(() => requirePermission(me(), Permission.USERS_READ));
		expect(isHttpError(error, 403)).toBe(true);
		expect(error).toMatchObject({ body: { message: t('error-forbidden') } });
	});

	it('sends signed-in users away from the guest pages', () => {
		expect(() => redirectIfSignedIn(null, new URL('http://app.test/login'))).not.toThrow();

		const redirect = thrown(() =>
			redirectIfSignedIn(me(), new URL('http://app.test/login?redirectTo=%2Fsettings%2Fprofile'))
		);
		expect(redirect).toMatchObject({ status: 307, location: '/settings/profile' });

		const home = thrown(() =>
			redirectIfSignedIn(me(), new URL('http://app.test/login?redirectTo=https://evil.example'))
		);
		expect(home).toMatchObject({ location: HOME });
	});
});

describe('handleApiError', () => {
	const forbidden = new ApiError(403, 'forbidden', 'no');

	it('refreshes the session when permissions may have changed, at most every 10 s', () => {
		let now = 0;
		const refreshSession = vi.fn(async () => {});
		const listener = handleApiError({ signedIn: () => true, refreshSession, now: () => now });

		listener(forbidden);
		listener(new ApiError(403, 'email_not_verified', 'verify'));
		expect(refreshSession).toHaveBeenCalledOnce();

		now = 10_000;
		listener(forbidden);
		expect(refreshSession).toHaveBeenCalledTimes(2);
	});

	it('leaves other errors, and signed-out visitors, alone', () => {
		const refreshSession = vi.fn(async () => {});
		handleApiError({ signedIn: () => true, refreshSession })(
			new ApiError(403, 'csrf_rejected', 'no')
		);
		handleApiError({ signedIn: () => false, refreshSession })(forbidden);
		expect(refreshSession).not.toHaveBeenCalled();
	});
});

describe('ReauthPrompt', () => {
	it('opens, and resolves to whether the user confirmed', async () => {
		const prompt = new ReauthPrompt();
		const first = prompt.request();
		expect(prompt.open).toBe(true);
		prompt.finish(true);
		await expect(first).resolves.toBe(true);
		expect(prompt.open).toBe(false);

		const stale = prompt.request();
		const fresh = prompt.request();
		await expect(stale).resolves.toBe(false);
		prompt.finish(false);
		await expect(fresh).resolves.toBe(false);
	});
});
