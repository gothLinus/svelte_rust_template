import {
	AddPhoneRequestSchema,
	AuditEventPageSchema,
	ChangeEmailRequestSchema,
	CodeRequestSchema,
	DeleteAccountRequestSchema,
	LinkedAccountListSchema,
	MeSchema,
	PasskeyAssertionRequestSchema,
	PasskeyCreationOptionsSchema,
	PasskeyListSchema,
	PasskeyRegisteredSchema,
	PasskeyRequestOptionsSchema,
	PasskeySchema,
	ReauthMethodsSchema,
	ReauthenticateRequestSchema,
	RecoveryCodesSchema,
	RegisterPasskeyRequestSchema,
	RenamePasskeyRequestSchema,
	SecondFactorAddedSchema,
	SecurityOverviewSchema,
	SessionListSchema,
	TotpSetupSchema,
	UpdateProfileRequestSchema,
	type Me,
	type User
} from '$lib/types/api';
import { fromBinary } from '@bufbuild/protobuf';
import { type ApiClient, type Decoder, type Init, blob, encode, message, segment } from './client';
import { ApiError } from './errors';

/**
 * `Me` as the server sends it: `user` is always set, though protobuf makes every message
 * field optional. The app uses this type, so pages read `me.user` without checks.
 */
export type SignedIn = Me & { user: User };

function hasUser(me: Me): me is SignedIn {
	return me.user !== undefined;
}

export const signedIn: Decoder<SignedIn> = (_status, body) => {
	const me = fromBinary(MeSchema, body);
	if (!hasUser(me)) throw new Error('`Me` without a user.');
	return me;
};

const me = signedIn;

export function meApi(client: ApiClient) {
	return {
		/**
		 * The signed-in user, or `null` when signed out. A 401 is the expected answer for
		 * anonymous visitors, so it does not trigger the unauthenticated handler.
		 */
		async current(): Promise<SignedIn | null> {
			try {
				return await client.get('/me', { response: me, handleUnauthenticated: false });
			} catch (error) {
				if (error instanceof ApiError && error.status === 401) return null;
				throw error;
			}
		},
		updateProfile: (body: Init<typeof UpdateProfileRequestSchema>) =>
			client.patch('/me', { body: encode(UpdateProfileRequestSchema, body), response: me }),
		changeEmail: (body: Init<typeof ChangeEmailRequestSchema>) =>
			client.post('/me/email', { body: encode(ChangeEmailRequestSchema, body) }),
		requestPasswordChange: () => client.post('/me/password'),
		addPhone: (body: Init<typeof AddPhoneRequestSchema>) =>
			client.post('/me/phone', { body: encode(AddPhoneRequestSchema, body) }),
		verifyPhone: (code: string) =>
			client.post('/me/phone/verify', { body: encode(CodeRequestSchema, { code }), response: me }),
		removePhone: () => client.delete('/me/phone', { response: me }),
		deleteAccount: (body: Init<typeof DeleteAccountRequestSchema>) =>
			client.delete('/me', { body: encode(DeleteAccountRequestSchema, body) }),
		/**
		 * Everything stored about the user, as a JSON file (their right of access). Needs a
		 * recent sign-in, which the step-up prompt takes care of.
		 */
		exportData: () =>
			client.get('/me/export', { accept: 'application/json', response: blob('application/json') }),
		sessions: async () =>
			(await client.get('/me/sessions', { response: message(SessionListSchema) })).sessions,
		revokeSession: (id: string) => client.delete(`/me/sessions/${segment(id)}`),
		/** The user's own security events, newest first. */
		activity: (query: { limit?: number; after?: string } = {}) =>
			client.get('/me/activity', { query: { ...query }, response: message(AuditEventPageSchema) }),

		security: () => client.get('/me/security', { response: message(SecurityOverviewSchema) }),
		startTotp: () => client.post('/me/mfa/totp', { response: message(TotpSetupSchema) }),
		confirmTotp: (code: string) =>
			client.post('/me/mfa/totp/confirm', {
				body: encode(CodeRequestSchema, { code }),
				response: message(SecondFactorAddedSchema)
			}),
		removeTotp: (code: string) =>
			client.delete('/me/mfa/totp', { body: encode(CodeRequestSchema, { code }) }),
		regenerateRecoveryCodes: () =>
			client.post('/me/mfa/recovery-codes', { response: message(RecoveryCodesSchema) }),

		passkeys: async () =>
			(await client.get('/me/passkeys', { response: message(PasskeyListSchema) })).passkeys,
		passkeyOptions: () =>
			client.post('/me/passkeys/options', { response: message(PasskeyCreationOptionsSchema) }),
		registerPasskey: (body: Init<typeof RegisterPasskeyRequestSchema>) =>
			client.post('/me/passkeys', {
				body: encode(RegisterPasskeyRequestSchema, body),
				response: message(PasskeyRegisteredSchema)
			}),
		renamePasskey: (id: string, name: string) =>
			client.patch(`/me/passkeys/${segment(id)}`, {
				body: encode(RenamePasskeyRequestSchema, { name }),
				response: message(PasskeySchema)
			}),
		deletePasskey: (id: string) => client.delete(`/me/passkeys/${segment(id)}`),

		reauthMethods: () =>
			client.get('/me/reauthenticate', { response: message(ReauthMethodsSchema) }),
		reauthenticate: (body: Init<typeof ReauthenticateRequestSchema>) =>
			client.post('/me/reauthenticate', { body: encode(ReauthenticateRequestSchema, body) }),
		reauthEmailCode: () => client.post('/me/reauthenticate/email-code'),
		reauthPasskeyOptions: () =>
			client.post('/me/reauthenticate/passkey/options', {
				response: message(PasskeyRequestOptionsSchema)
			}),
		reauthPasskey: (body: Init<typeof PasskeyAssertionRequestSchema>) =>
			client.post('/me/reauthenticate/passkey', {
				body: encode(PasskeyAssertionRequestSchema, body)
			}),

		linkedAccounts: async () =>
			(await client.get('/me/linked-accounts', { response: message(LinkedAccountListSchema) }))
				.accounts,
		unlink: (provider: string) => client.delete(`/me/linked-accounts/${segment(provider)}`)
	};
}
