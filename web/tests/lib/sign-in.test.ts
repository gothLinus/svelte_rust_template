import { create } from '@bufbuild/protobuf';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import {
	AuthMethodsSchema,
	MfaMethod,
	PasskeyCreationOptionsSchema,
	PasskeyRequestOptionsSchema,
	TextChannel
} from '$lib/types/api';
import { t } from '$lib/i18n';
import {
	DEFAULT_EMAIL_CHANGE_HOURS,
	DEFAULT_EMAIL_VERIFICATION_HOURS,
	DEFAULT_PASSWORD_RESET_MINUTES,
	DEFAULT_SIGN_IN_MINUTES,
	DEFAULT_TEXT_CODE_MINUTES
} from '$lib/types/generated/limits';
import { me, methods, mockFetch, problem, reply } from '../helpers';

const goto = vi.fn();
vi.mock('$app/navigation', () => ({ goto: (...args: unknown[]) => goto(...args) }));

const {
	PasskeyCancelled,
	createPasskey,
	finishSignIn,
	lifetimes,
	loadAuthMethods,
	mfaUrl,
	oauthErrorMessage,
	passkeysSupported,
	usePasskey
} = await import('$lib/auth');

beforeEach(() => {
	goto.mockReset();
});

describe('finishing a sign-in', () => {
	it('reloads the session and goes where the user was headed', async () => {
		await finishSignIn({ kind: 'signedIn', me: me() }, '/notes?page=2');
		expect(goto).toHaveBeenCalledWith('/notes?page=2', { invalidateAll: true });
	});

	it('never follows a redirect off the site', async () => {
		await finishSignIn({ kind: 'signedIn', me: me() }, '//evil.example');
		expect(goto).toHaveBeenCalledWith('/dashboard', { invalidateAll: true });
	});

	it('sends a second step to its page', async () => {
		await finishSignIn({ kind: 'mfa', methods: [MfaMethod.TOTP] }, '/admin/users');
		expect(goto).toHaveBeenCalledWith('/login/mfa?redirectTo=%2Fadmin%2Fusers');
		expect(mfaUrl(null)).toBe('/login/mfa?redirectTo=%2Fdashboard');
	});
});

describe('sign-in methods', () => {
	it('loads them, or offers nothing extra when the server cannot say', async () => {
		const offered = {
			appName: 'Widgets',
			providers: [{ id: 'github', name: 'GitHub' }],
			textChannels: [TextChannel.SMS]
		};
		await expect(loadAuthMethods(mockFetch(reply(AuthMethodsSchema, offered)))).resolves.toEqual(
			methods(offered)
		);
		await expect(loadAuthMethods(mockFetch(problem(500, 'internal_error')))).resolves.toEqual(
			create(AuthMethodsSchema, { appName: 'App' })
		);
	});

	it('says how long links and codes work: as configured, else the defaults', () => {
		const configured = methods({
			lifetimes: {
				signInMinutes: 5,
				textCodeMinutes: 3,
				passwordResetMinutes: 45,
				emailVerificationHours: 48,
				emailChangeHours: 2
			}
		});
		expect(lifetimes(configured)).toBe(configured.lifetimes);
		for (const fallback of [lifetimes(methods()), lifetimes(undefined)]) {
			expect(fallback).toMatchObject({
				signInMinutes: DEFAULT_SIGN_IN_MINUTES,
				textCodeMinutes: DEFAULT_TEXT_CODE_MINUTES,
				passwordResetMinutes: DEFAULT_PASSWORD_RESET_MINUTES,
				emailVerificationHours: DEFAULT_EMAIL_VERIFICATION_HOURS,
				emailChangeHours: DEFAULT_EMAIL_CHANGE_HOURS
			});
		}
	});

	it('explains social sign-in failures', () => {
		expect(oauthErrorMessage(null)).toBeNull();
		expect(oauthErrorMessage('email_in_use')).toBe(t('signin-oauth-error-email-in-use'));
		expect(oauthErrorMessage('email_unverified')).toBe(t('signin-oauth-error-email-unverified'));
		expect(oauthErrorMessage('something_new')).toBe(t('signin-oauth-error-failed'));
	});

	it.each([
		'oauth_cancelled',
		'oauth_state_invalid',
		'email_in_use',
		'email_required',
		'provider_unavailable',
		'identity_taken',
		'provider_linked',
		'account_disabled',
		'email_not_verified',
		'email_unverified',
		'reauth_required',
		'rate_limited',
		'not_found',
		'unauthenticated'
	])('has its own message for %s', (code) => {
		expect(oauthErrorMessage(code)).toBe(t(`signin-oauth-error-${code.replaceAll('_', '-')}`));
		expect(oauthErrorMessage(code)).not.toBe(t('signin-oauth-error-failed'));
	});
});

describe('passkeys in the browser', () => {
	it('reports missing support outside a browser', () => {
		expect(passkeysSupported()).toBe(false);
	});

	function fakeBrowser(userHandle: ArrayBuffer | null = new Uint8Array([7]).buffer) {
		class FakeCredential {
			rawId = new Uint8Array([1, 2]).buffer;
			constructor(readonly response: object) {}
		}
		const buffer = (value: number) => new Uint8Array([value]).buffer;
		const create = vi.fn<(options: CredentialCreationOptions) => Promise<FakeCredential | null>>(
			async () =>
				new FakeCredential({
					clientDataJSON: buffer(3),
					getAuthenticatorData: () => buffer(4),
					getPublicKey: () => buffer(5),
					getPublicKeyAlgorithm: () => -7,
					getTransports: () => ['internal']
				})
		);
		const get = vi.fn<(options: CredentialRequestOptions) => Promise<FakeCredential | null>>(
			async () =>
				new FakeCredential({
					clientDataJSON: buffer(3),
					authenticatorData: buffer(4),
					signature: buffer(6),
					userHandle
				})
		);
		vi.stubGlobal('PublicKeyCredential', FakeCredential);
		vi.stubGlobal('navigator', { credentials: { create, get } });
		return { create, get, FakeCredential };
	}

	const creationOptions = () =>
		create(PasskeyCreationOptionsSchema, {
			challengeId: 'c1',
			publicKey: {
				rp: { id: 'localhost', name: 'Acme' },
				user: { id: new Uint8Array([1]), name: 'alice', displayName: 'Alice' },
				challenge: new Uint8Array([2]),
				algorithms: [-7],
				timeout: 300000,
				excludeCredentials: [{ id: new Uint8Array([3]), transports: ['usb'] }],
				authenticatorSelection: { residentKey: 'preferred', userVerification: 'preferred' },
				attestation: 'none'
			}
		});

	const requestOptions = (challengeId = 'c2') =>
		create(PasskeyRequestOptionsSchema, {
			challengeId,
			publicKey: {
				challenge: new Uint8Array([2]),
				rpId: 'localhost',
				timeout: 300000,
				userVerification: 'required',
				allowCredentials: [{ id: new Uint8Array([3]), transports: ['internal'] }]
			}
		});

	it('passes the bytes through unchanged, both ways', async () => {
		const { create: createCredential, get } = fakeBrowser();

		const registration = await createPasskey(creationOptions(), 'MacBook');
		const publicKey = createCredential.mock.calls[0]?.[0].publicKey;
		expect(publicKey?.user.id).toEqual(new Uint8Array([1]));
		expect(publicKey?.challenge).toEqual(new Uint8Array([2]));
		expect(publicKey?.pubKeyCredParams).toEqual([{ type: 'public-key', alg: -7 }]);
		expect(publicKey?.excludeCredentials).toEqual([
			{ type: 'public-key', id: new Uint8Array([3]), transports: ['usb'] }
		]);
		expect(publicKey?.authenticatorSelection).toEqual({
			residentKey: 'preferred',
			userVerification: 'preferred'
		});
		expect(registration).toEqual({
			challengeId: 'c1',
			name: 'MacBook',
			credentialId: new Uint8Array([1, 2]),
			clientDataJson: new Uint8Array([3]),
			authenticatorData: new Uint8Array([4]),
			publicKey: new Uint8Array([5]),
			publicKeyAlgorithm: -7,
			transports: ['internal']
		});

		const assertion = await usePasskey(requestOptions());
		const request = get.mock.calls[0]?.[0].publicKey;
		expect(request?.challenge).toEqual(new Uint8Array([2]));
		expect(request?.allowCredentials).toEqual([
			{ type: 'public-key', id: new Uint8Array([3]), transports: ['internal'] }
		]);
		expect(assertion).toEqual({
			challengeId: 'c2',
			credentialId: new Uint8Array([1, 2]),
			clientDataJson: new Uint8Array([3]),
			authenticatorData: new Uint8Array([4]),
			signature: new Uint8Array([6]),
			userHandle: new Uint8Array([7])
		});
		vi.unstubAllGlobals();
	});

	it('leaves out a missing user handle', async () => {
		fakeBrowser(null);
		await expect(usePasskey(requestOptions())).resolves.toMatchObject({
			userHandle: undefined
		});
		vi.unstubAllGlobals();
	});

	it('treats a closed prompt as cancelled', async () => {
		const { create: createCredential, get } = fakeBrowser();
		get.mockRejectedValueOnce(new DOMException('closed', 'NotAllowedError'));
		await expect(usePasskey(requestOptions('c3'))).rejects.toBeInstanceOf(PasskeyCancelled);
		createCredential.mockRejectedValueOnce(new DOMException('aborted', 'AbortError'));
		await expect(createPasskey(creationOptions(), 'Key')).rejects.toBeInstanceOf(PasskeyCancelled);
		vi.unstubAllGlobals();
	});

	it('reports other failures, and an authenticator it cannot use', async () => {
		const { create: createCredential, get, FakeCredential } = fakeBrowser();
		get.mockRejectedValueOnce(new DOMException('bad origin', 'SecurityError'));
		await expect(usePasskey(requestOptions())).rejects.toThrow('bad origin');
		get.mockResolvedValueOnce(null);
		await expect(usePasskey(requestOptions())).rejects.toBeInstanceOf(PasskeyCancelled);

		createCredential.mockRejectedValueOnce(new DOMException('bad origin', 'SecurityError'));
		await expect(createPasskey(creationOptions(), 'Key')).rejects.toThrow('bad origin');
		createCredential.mockResolvedValueOnce(null);
		await expect(createPasskey(creationOptions(), 'Key')).rejects.toBeInstanceOf(PasskeyCancelled);
		createCredential.mockResolvedValueOnce(new FakeCredential({ getPublicKey: () => null }));
		await expect(createPasskey(creationOptions(), 'Key')).rejects.toThrow('unsupported key type');
		vi.unstubAllGlobals();
	});

	it('rejects options without what the ceremony needs', async () => {
		fakeBrowser();
		await expect(
			createPasskey(create(PasskeyCreationOptionsSchema, { challengeId: 'c' }), 'Key')
		).rejects.toThrow('incomplete');
		await expect(
			usePasskey(create(PasskeyRequestOptionsSchema, { challengeId: 'c' }))
		).rejects.toThrow('incomplete');
		vi.unstubAllGlobals();
	});
});
