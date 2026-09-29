import {
	AuthMethodsSchema,
	CancelEmailChangeRequestSchema,
	CodeRequestSchema,
	ConfirmEmailRequestSchema,
	EmailCodeRequestSchema,
	ForgotPasswordRequestSchema,
	LoginRequestSchema,
	MagicLinkRequestSchema,
	MfaChallengeSchema,
	PasskeyAssertionRequestSchema,
	PasskeyRequestOptionsSchema,
	PhoneCodeRequestSchema,
	RegisterRequestSchema,
	ResetPasswordRequestSchema,
	VerificationPendingSchema,
	VerifyEmailCodeRequestSchema,
	VerifyEmailRequestSchema,
	VerifyPhoneCodeRequestSchema,
	type MfaMethod
} from '$lib/types/api';
import { fromBinary } from '@bufbuild/protobuf';
import { type SignedIn, signedIn } from './me';
import {
	API_BASE,
	type ApiClient,
	type Decoder,
	type Init,
	encode,
	message,
	searchParams,
	segment
} from './client';

export type RegisterResult =
	{ kind: 'signedIn'; me: SignedIn } | { kind: 'verificationPending'; email: string };

/**
 * A first sign-in step either signs in, or asks for a second step (an authenticator app, a
 * passkey or a recovery code). The server keeps track of the attempt in a cookie.
 */
export type LoginResult =
	{ kind: 'signedIn'; me: SignedIn } | { kind: 'mfa'; methods: MfaMethod[] };

const loginResult: Decoder<LoginResult> = (status, body) =>
	status === 202
		? { kind: 'mfa', methods: fromBinary(MfaChallengeSchema, body).methods }
		: { kind: 'signedIn', me: signedIn(status, body) };

const registerResult: Decoder<RegisterResult> = (status, body) =>
	status === 202
		? { kind: 'verificationPending', email: fromBinary(VerificationPendingSchema, body).email }
		: { kind: 'signedIn', me: signedIn(status, body) };

const me = signedIn;
const passkeyOptions = message(PasskeyRequestOptionsSchema);

// A 401 from a sign-in endpoint means wrong credentials, not an expired session.
const signIn = { handleUnauthenticated: false } as const;

export function authApi(client: ApiClient) {
	return {
		methods: () => client.get('/auth/methods', { response: message(AuthMethodsSchema) }),
		register: (body: Init<typeof RegisterRequestSchema>) =>
			client.post('/auth/register', {
				body: encode(RegisterRequestSchema, body),
				response: registerResult
			}),
		login: (body: Init<typeof LoginRequestSchema>) =>
			client.post('/auth/login', {
				body: encode(LoginRequestSchema, body),
				response: loginResult,
				...signIn
			}),
		logout: () => client.post('/auth/logout'),
		logoutAll: () => client.post('/auth/logout-all'),
		verifyEmail: (body: Init<typeof VerifyEmailRequestSchema>) =>
			client.post('/auth/verify-email', { body: encode(VerifyEmailRequestSchema, body) }),
		resendVerification: () => client.post('/auth/verify-email/resend'),
		forgotPassword: (body: Init<typeof ForgotPasswordRequestSchema>) =>
			client.post('/auth/forgot-password', { body: encode(ForgotPasswordRequestSchema, body) }),
		resetPassword: (body: Init<typeof ResetPasswordRequestSchema>) =>
			client.post('/auth/reset-password', { body: encode(ResetPasswordRequestSchema, body) }),
		confirmEmail: (body: Init<typeof ConfirmEmailRequestSchema>) =>
			client.post('/auth/confirm-email', { body: encode(ConfirmEmailRequestSchema, body) }),
		cancelEmailChange: (body: Init<typeof CancelEmailChangeRequestSchema>) =>
			client.post('/auth/cancel-email-change', {
				body: encode(CancelEmailChangeRequestSchema, body)
			}),

		emailCode: (body: Init<typeof EmailCodeRequestSchema>) =>
			client.post('/auth/email-code', { body: encode(EmailCodeRequestSchema, body) }),
		verifyEmailCode: (body: Init<typeof VerifyEmailCodeRequestSchema>) =>
			client.post('/auth/email-code/verify', {
				body: encode(VerifyEmailCodeRequestSchema, body),
				response: loginResult,
				...signIn
			}),
		magicLink: (body: Init<typeof MagicLinkRequestSchema>) =>
			client.post('/auth/magic-link', {
				body: encode(MagicLinkRequestSchema, body),
				response: loginResult,
				...signIn
			}),
		phoneCode: (body: Init<typeof PhoneCodeRequestSchema>) =>
			client.post('/auth/phone-code', { body: encode(PhoneCodeRequestSchema, body) }),
		verifyPhoneCode: (body: Init<typeof VerifyPhoneCodeRequestSchema>) =>
			client.post('/auth/phone-code/verify', {
				body: encode(VerifyPhoneCodeRequestSchema, body),
				response: loginResult,
				...signIn
			}),

		passkeyOptions: () => client.post('/auth/passkeys/options', { response: passkeyOptions }),
		passkeyLogin: (body: Init<typeof PasskeyAssertionRequestSchema>) =>
			client.post('/auth/passkeys/login', {
				body: encode(PasskeyAssertionRequestSchema, body),
				response: me,
				...signIn
			}),

		mfaPending: () => client.get('/auth/mfa', { response: message(MfaChallengeSchema), ...signIn }),
		mfaTotp: (code: string) =>
			client.post('/auth/mfa/totp', {
				body: encode(CodeRequestSchema, { code }),
				response: me,
				...signIn
			}),
		mfaRecoveryCode: (code: string) =>
			client.post('/auth/mfa/recovery-code', {
				body: encode(CodeRequestSchema, { code }),
				response: me,
				...signIn
			}),
		mfaPasskeyOptions: () =>
			client.post('/auth/mfa/passkeys/options', { response: passkeyOptions, ...signIn }),
		mfaPasskey: (body: Init<typeof PasskeyAssertionRequestSchema>) =>
			client.post('/auth/mfa/passkeys', {
				body: encode(PasskeyAssertionRequestSchema, body),
				response: me,
				...signIn
			})
	};
}

/**
 * Where to send the browser to sign in with a social provider. A full page navigation,
 * not a `fetch`: the provider's page takes over and redirects back to the API.
 */
export function oauthUrl(provider: string, redirectTo?: string | null): string {
	return `${API_BASE}/auth/oauth/${segment(provider)}${searchParams({ redirectTo })}`;
}

export function oauthLinkUrl(provider: string): string {
	return `${API_BASE}/auth/oauth/${segment(provider)}/link`;
}
