import { create } from '@bufbuild/protobuf';
import { afterEach, describe, expect, it, vi } from 'vitest';
import {
	API_BASE,
	ApiError,
	INVALID_RESPONSE,
	UPLOAD_TIMEOUT_MS,
	contentUrl,
	createApi,
	oauthLinkUrl,
	oauthUrl,
	setReauthenticator
} from '$lib/api';
import {
	FileUsageSchema,
	LoginRequestSchema,
	MeSchema,
	MfaChallengeSchema,
	MfaMethod,
	NotePageSchema,
	ReauthMethod,
	ReauthenticateRequestSchema,
	RoleListSchema,
	SessionListSchema,
	SetLocaleRequestSchema,
	StoredFilePageSchema,
	StoredFileSchema,
	TextChannel,
	UpdateFileRequestSchema,
	UpdateNoteRequestSchema,
	UserPageSchema,
	UserSchema,
	VerificationPendingSchema
} from '$lib/types/api';
import {
	callOf,
	me,
	mockFetch,
	noContent,
	note,
	problem,
	reply,
	sent,
	storedFile
} from '../helpers';

const ok = noContent;
const signedIn = (status = 200) => reply(MeSchema, me(), status);
const empty = () => reply(UserSchema);

describe('auth', () => {
	it('registers and signs in', async () => {
		const fetchFn = mockFetch(signedIn(201));

		const result = await createApi(fetchFn).auth.register({
			email: 'a@example.com',
			username: 'alice',
			password: 'correct horse'
		});

		expect(result).toEqual({ kind: 'signedIn', me: me() });
		expect(callOf(fetchFn)[0]).toBe(`${API_BASE}/auth/register`);
	});

	it('registers pending verification', async () => {
		const fetchFn = mockFetch(reply(VerificationPendingSchema, { email: 'a@example.com' }, 202));

		const result = await createApi(fetchFn).auth.register({
			email: 'a@example.com',
			username: 'alice',
			password: 'correct horse'
		});

		expect(result).toEqual({ kind: 'verificationPending', email: 'a@example.com' });
	});

	it('calls the right endpoints', async () => {
		const fetchFn = mockFetch(signedIn(), ok());
		const api = createApi(fetchFn);

		await api.auth.login({ identifier: 'a@example.com', password: 'x' });
		await api.auth.logout();
		await api.auth.logoutAll();
		await api.auth.verifyEmail({ token: 't' });
		await api.auth.resendVerification();
		await api.auth.forgotPassword({ email: 'a@example.com' });
		await api.auth.resetPassword({ token: 't', password: 'correct horse' });

		expect(fetchFn.mock.calls.map(([url]) => String(url).replace(API_BASE, ''))).toEqual([
			'/auth/login',
			'/auth/logout',
			'/auth/logout-all',
			'/auth/verify-email',
			'/auth/verify-email/resend',
			'/auth/forgot-password',
			'/auth/reset-password'
		]);
		expect(sent(fetchFn, LoginRequestSchema)).toEqual(
			create(LoginRequestSchema, { identifier: 'a@example.com', password: 'x' })
		);
	});

	it('tells a session from a second step', async () => {
		const fetchFn = mockFetch(signedIn());
		await expect(
			createApi(fetchFn).auth.login({ identifier: 'alice', password: 'x' })
		).resolves.toEqual({ kind: 'signedIn', me: me() });

		const methods = [MfaMethod.TOTP, MfaMethod.RECOVERY_CODE];
		const mfa = mockFetch(reply(MfaChallengeSchema, { methods }, 202));
		await expect(
			createApi(mfa).auth.verifyEmailCode({ email: 'a@example.com', code: '123456' })
		).resolves.toEqual({ kind: 'mfa', methods });
	});

	it('rejects a session without its user', async () => {
		const fetchFn = mockFetch(reply(MeSchema, { permissions: [] }));
		await expect(
			createApi(fetchFn).auth.login({ identifier: 'alice', password: 'x' })
		).rejects.toMatchObject({ code: INVALID_RESPONSE });
	});

	it('calls the passwordless, passkey and second-step endpoints', async () => {
		// Answers in call order: `Me` where a call signs in, an empty message elsewhere.
		const fetchFn = mockFetch(
			...[empty(), empty(), signedIn(), empty(), signedIn(), empty(), signedIn(), empty()],
			...[signedIn(), signedIn(), empty(), signedIn(), empty()]
		);
		const api = createApi(fetchFn);
		const bytes = new Uint8Array([1]);
		const assertion = {
			challengeId: 'c',
			credentialId: bytes,
			clientDataJson: bytes,
			authenticatorData: bytes,
			signature: bytes
		};

		await api.auth.methods();
		await api.auth.emailCode({ email: 'a@example.com' });
		await api.auth.magicLink({ token: 't' });
		await api.auth.phoneCode({ phone: '+15551234567', channel: TextChannel.SMS });
		await api.auth.verifyPhoneCode({ phone: '+15551234567', code: '123456' });
		await api.auth.passkeyOptions();
		await api.auth.passkeyLogin(assertion);
		await api.auth.mfaPending();
		await api.auth.mfaTotp('123456');
		await api.auth.mfaRecoveryCode('abcde-fghjk');
		await api.auth.mfaPasskeyOptions();
		await api.auth.mfaPasskey(assertion);
		await api.auth.confirmEmail({ token: 't' });

		expect(
			fetchFn.mock.calls.map(
				([url, init]) => `${init?.method} ${String(url).replace(API_BASE, '')}`
			)
		).toEqual([
			'GET /auth/methods',
			'POST /auth/email-code',
			'POST /auth/magic-link',
			'POST /auth/phone-code',
			'POST /auth/phone-code/verify',
			'POST /auth/passkeys/options',
			'POST /auth/passkeys/login',
			'GET /auth/mfa',
			'POST /auth/mfa/totp',
			'POST /auth/mfa/recovery-code',
			'POST /auth/mfa/passkeys/options',
			'POST /auth/mfa/passkeys',
			'POST /auth/confirm-email'
		]);
	});

	it('builds social sign-in URLs', () => {
		expect(oauthUrl('github', '/notes?x=1')).toBe(
			`${API_BASE}/auth/oauth/github?redirectTo=%2Fnotes%3Fx%3D1`
		);
		expect(oauthUrl('google')).toBe(`${API_BASE}/auth/oauth/google`);
		expect(oauthLinkUrl('apple')).toBe(`${API_BASE}/auth/oauth/apple/link`);
	});
});

describe('me', () => {
	afterEach(() => setReauthenticator(null));

	const exported = () =>
		new Response('{"account":{"email":"alice@example.com"}}', {
			headers: {
				'content-type': 'application/json',
				'content-disposition': 'attachment; filename="personal-data.json"'
			}
		});

	it('downloads the data export as a JSON blob', async () => {
		const fetchFn = mockFetch(exported());
		const file = await createApi(fetchFn).me.exportData();

		expect(file.type).toBe('application/json');
		expect(JSON.parse(await file.text())).toEqual({ account: { email: 'alice@example.com' } });
		const [url, init] = callOf(fetchFn);
		expect(url).toBe(`${API_BASE}/me/export`);
		expect(init.method).toBe('GET');
		const headers = init.headers as Record<string, string>;
		expect(headers.accept).toBe('application/json, application/problem+json');
		expect(headers['x-requested-with']).toBe('fetch');
	});

	it('confirms it is the user before exporting, then exports', async () => {
		const reauthenticator = vi.fn(async () => true);
		setReauthenticator(reauthenticator);
		const fetchFn = mockFetch(problem(403, 'reauth_required'), exported());

		const file = await createApi(fetchFn).me.exportData();
		expect(reauthenticator).toHaveBeenCalledOnce();
		expect(fetchFn).toHaveBeenCalledTimes(2);
		expect(file.size).toBeGreaterThan(0);
	});

	it('refuses an export that is not JSON', async () => {
		const html = new Response('<html></html>', { headers: { 'content-type': 'text/html' } });
		await expect(createApi(mockFetch(html)).me.exportData()).rejects.toMatchObject({
			code: INVALID_RESPONSE
		});
	});

	it('returns null when signed out', async () => {
		const fetchFn = mockFetch(problem(401, 'unauthenticated'));
		await expect(createApi(fetchFn).me.current()).resolves.toBeNull();
	});

	it('returns the signed-in user', async () => {
		await expect(createApi(mockFetch(signedIn())).me.current()).resolves.toEqual(me());
	});

	it('rethrows other failures', async () => {
		await expect(
			createApi(mockFetch(problem(500, 'internal_error'))).me.current()
		).rejects.toBeInstanceOf(ApiError);
	});

	it('calls the right endpoints', async () => {
		// Answers in call order: `Me` where a call returns it, an empty message elsewhere.
		const fetchFn = mockFetch(
			...[signedIn(), ok(), ok(), ok(), signedIn(), signedIn(), ok()],
			reply(SessionListSchema, { sessions: [{ id: 's1', current: true }] }),
			empty()
		);
		const api = createApi(fetchFn);

		await api.me.updateProfile({ username: 'bob' });
		await api.me.requestPasswordChange();
		await api.me.changeEmail({ email: 'b@example.com' });
		await api.me.addPhone({ phone: '+15551234567', channel: TextChannel.WHATSAPP });
		await api.me.verifyPhone('123456');
		await api.me.removePhone();
		await api.me.deleteAccount({ password: 'a' });
		const sessions = await api.me.sessions();
		await api.me.revokeSession('a/b');
		await api.me.security();
		await api.me.startTotp();
		await api.me.confirmTotp('123456');
		await api.me.removeTotp('123456');
		await api.me.regenerateRecoveryCodes();
		await api.me.passkeys();
		await api.me.passkeyOptions();
		await api.me.renamePasskey('p', 'Key');
		await api.me.deletePasskey('p');
		await api.me.linkedAccounts();
		await api.me.unlink('github');
		await api.me.reauthMethods();
		await api.me.reauthenticate({ method: ReauthMethod.TOTP, secret: '123456' });
		await api.me.reauthEmailCode();
		await api.me.reauthPasskeyOptions();
		await api.me.reauthPasskey({ challengeId: 'c' });
		await api.me.registerPasskey({ challengeId: 'c', name: 'Key' });
		await api.me.activity({ limit: 3, after: 'c1' });

		expect(sessions.map((session) => session.id)).toEqual(['s1']);
		expect(sent(fetchFn, ReauthenticateRequestSchema, 21)).toEqual(
			create(ReauthenticateRequestSchema, { method: ReauthMethod.TOTP, secret: '123456' })
		);

		expect(
			fetchFn.mock.calls.map(
				([url, init]) => `${init?.method} ${String(url).replace(API_BASE, '')}`
			)
		).toEqual([
			'PATCH /me',
			'POST /me/password',
			'POST /me/email',
			'POST /me/phone',
			'POST /me/phone/verify',
			'DELETE /me/phone',
			'DELETE /me',
			'GET /me/sessions',
			'DELETE /me/sessions/a%2Fb',
			'GET /me/security',
			'POST /me/mfa/totp',
			'POST /me/mfa/totp/confirm',
			'DELETE /me/mfa/totp',
			'POST /me/mfa/recovery-codes',
			'GET /me/passkeys',
			'POST /me/passkeys/options',
			'PATCH /me/passkeys/p',
			'DELETE /me/passkeys/p',
			'GET /me/linked-accounts',
			'DELETE /me/linked-accounts/github',
			'GET /me/reauthenticate',
			'POST /me/reauthenticate',
			'POST /me/reauthenticate/email-code',
			'POST /me/reauthenticate/passkey/options',
			'POST /me/reauthenticate/passkey',
			'POST /me/passkeys',
			'GET /me/activity?limit=3&after=c1'
		]);
	});

	it('sets the language, or clears it', async () => {
		const fetchFn = mockFetch(signedIn());
		const api = createApi(fetchFn);

		await api.me.setLocale('de');
		await api.me.setLocale();

		expect(callOf(fetchFn)[0]).toBe(`${API_BASE}/me/locale`);
		expect(callOf(fetchFn)[1].method).toBe('PUT');
		expect(sent(fetchFn, SetLocaleRequestSchema).locale).toBe('de');
		expect(sent(fetchFn, SetLocaleRequestSchema, 1).locale).toBeUndefined();
	});
});

describe('notes', () => {
	it('lists with a query and manages single notes', async () => {
		const fetchFn = mockFetch(reply(NotePageSchema, { items: [note()] }), empty());
		const api = createApi(fetchFn);

		const page = await api.notes.list({ scope: 'all', limit: 5 });
		await api.notes.list();
		await api.notes.get('n1');
		await api.notes.create({ title: 'x' });
		await api.notes.update('n1', '3', { body: 'y' });
		await api.notes.remove('n1', '4');
		await api.notes.duplicate('n1');

		expect(page.items).toEqual([note()]);
		expect(page.nextCursor).toBeUndefined();
		expect(sent(fetchFn, UpdateNoteRequestSchema, 4)).toEqual(
			create(UpdateNoteRequestSchema, { body: 'y' })
		);
		expect(callOf(fetchFn, 4)[1].headers).toMatchObject({ 'if-match': '"3"' });
		expect(callOf(fetchFn, 5)[1].headers).toMatchObject({ 'if-match': '"4"' });
		expect(
			fetchFn.mock.calls.map(
				([url, init]) => `${init?.method} ${String(url).replace(API_BASE, '')}`
			)
		).toEqual([
			'GET /notes?scope=all&limit=5',
			'GET /notes',
			'GET /notes/n1',
			'POST /notes',
			'PATCH /notes/n1',
			'DELETE /notes/n1',
			'POST /notes/n1/duplicate'
		]);
	});
});

describe('files', () => {
	it('lists, reads, renames and removes files', async () => {
		const fetchFn = mockFetch(
			reply(StoredFilePageSchema, { items: [storedFile()], nextCursor: 'c1' }),
			reply(StoredFileSchema, storedFile()),
			reply(FileUsageSchema, { usedBytes: 10n, quotaBytes: 100n }),
			reply(StoredFileSchema, storedFile(undefined, { name: 'q3.pdf' })),
			ok()
		);
		const api = createApi(fetchFn);

		const page = await api.files.list({ scope: 'all', limit: 5, after: 'c0' });
		await api.files.get('f1');
		const usage = await api.files.usage();
		const renamed = await api.files.rename('f1', 'q3.pdf');
		await api.files.remove('f1');

		expect(page.items).toEqual([storedFile()]);
		expect(page.nextCursor).toBe('c1');
		expect(usage.usedBytes).toBe(10n);
		expect(usage.quotaBytes).toBe(100n);
		expect(renamed.name).toBe('q3.pdf');
		expect(sent(fetchFn, UpdateFileRequestSchema, 3)).toEqual(
			create(UpdateFileRequestSchema, { name: 'q3.pdf' })
		);
		expect(
			fetchFn.mock.calls.map(
				([url, init]) => `${init?.method} ${String(url).replace(API_BASE, '')}`
			)
		).toEqual([
			'GET /files?scope=all&limit=5&after=c0',
			'GET /files/f1',
			'GET /files/usage',
			'PATCH /files/f1',
			'DELETE /files/f1'
		]);
		expect(contentUrl('a/b')).toBe(`${API_BASE}/files/a%2Fb/content`);
	});

	it('uploads the file itself as the body, with its type and a longer timeout', async () => {
		const timeout = vi.spyOn(AbortSignal, 'timeout');
		const fetchFn = mockFetch(reply(StoredFileSchema, storedFile(), 201));
		const file = new File(['%PDF'], 'ignored.pdf', { type: 'application/pdf' });

		const stored = await createApi(fetchFn).files.upload(file, 'Q3 report.pdf');
		const timeouts = timeout.mock.calls.map(([ms]) => ms);
		timeout.mockRestore();

		expect(stored).toEqual(storedFile());
		const [url, init] = callOf(fetchFn);
		expect(url).toBe(`${API_BASE}/files?name=Q3+report.pdf`);
		expect(init.method).toBe('POST');
		expect(init.body).toBe(file);
		expect(new Headers(init.headers).get('content-type')).toBe('application/pdf');
		expect(new Headers(init.headers).get('x-requested-with')).toBe('fetch');
		// The whole file must arrive before it: far longer than any other request's.
		expect(timeouts).toEqual([UPLOAD_TIMEOUT_MS]);
		expect(UPLOAD_TIMEOUT_MS).toBe(10 * 60 * 1000);
	});

	it('uploads a file of unknown type as octet-stream', async () => {
		const fetchFn = mockFetch(reply(StoredFileSchema, storedFile(), 201));

		await createApi(fetchFn).files.upload(new Blob(['x']), 'data');

		expect(new Headers(callOf(fetchFn)[1].headers).get('content-type')).toBe(
			'application/octet-stream'
		);
	});
});

describe('admin', () => {
	it('calls the right endpoints', async () => {
		const fetchFn = mockFetch(
			reply(UserPageSchema, { nextCursor: 'c1' }),
			empty(),
			empty(),
			reply(RoleListSchema, { roles: [{ name: 'admin' }] }),
			empty()
		);
		const api = createApi(fetchFn);

		await expect(api.admin.users({ search: 'ali', limit: 10 })).resolves.toMatchObject({
			nextCursor: 'c1'
		});
		await api.admin.users();
		await api.admin.user('u1');
		await expect(api.admin.roles()).resolves.toMatchObject([{ name: 'admin' }]);
		await api.admin.grantRole('u1', 'admin');
		await api.admin.revokeRole('u1', 'admin');
		await api.admin.disable('u1');
		await api.admin.enable('u1');
		await api.admin.sessions('u1');
		await api.admin.revokeSession('u1', 's1');
		await api.admin.signOut('u1');
		await api.admin.audit({ user: 'u1', limit: 5 });
		await api.admin.audit();

		expect(
			fetchFn.mock.calls.map(
				([url, init]) => `${init?.method} ${String(url).replace(API_BASE, '')}`
			)
		).toEqual([
			'GET /admin/users?search=ali&limit=10',
			'GET /admin/users',
			'GET /admin/users/u1',
			'GET /admin/roles',
			'PUT /admin/users/u1/roles/admin',
			'DELETE /admin/users/u1/roles/admin',
			'POST /admin/users/u1/disable',
			'POST /admin/users/u1/enable',
			'GET /admin/users/u1/sessions',
			'DELETE /admin/users/u1/sessions/s1',
			'DELETE /admin/users/u1/sessions',
			'GET /admin/audit?user=u1&limit=5',
			'GET /admin/audit'
		]);
	});
});
