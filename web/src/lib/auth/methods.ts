import { createApi } from '$lib/api';
import { t } from '$lib/i18n';
import {
	type AuthMethods,
	AuthMethodsSchema,
	type LinkLifetimes,
	LinkLifetimesSchema
} from '$lib/types/api';
import {
	DEFAULT_EMAIL_CHANGE_HOURS,
	DEFAULT_EMAIL_VERIFICATION_HOURS,
	DEFAULT_PASSWORD_RESET_MINUTES,
	DEFAULT_SIGN_IN_MINUTES,
	DEFAULT_TEXT_CODE_MINUTES
} from '$lib/types/generated/limits';
import { create } from '@bufbuild/protobuf';

export const FALLBACK_APP_NAME = 'App';

/** The app's name and the sign-in methods on offer. Nothing extra when the server cannot be asked. */
export async function loadAuthMethods(fetchFn: typeof fetch): Promise<AuthMethods> {
	try {
		return await createApi(fetchFn).auth.methods();
	} catch {
		return create(AuthMethodsSchema, { appName: FALLBACK_APP_NAME });
	}
}

/**
 * How long the links and codes the server sends keep working, for the texts that promise
 * a deadline: what the server is configured with, or its defaults (generated from the
 * domain) while it cannot be asked.
 */
export function lifetimes(methods: AuthMethods | undefined): LinkLifetimes {
	return (
		methods?.lifetimes ??
		create(LinkLifetimesSchema, {
			signInMinutes: DEFAULT_SIGN_IN_MINUTES,
			textCodeMinutes: DEFAULT_TEXT_CODE_MINUTES,
			passwordResetMinutes: DEFAULT_PASSWORD_RESET_MINUTES,
			emailVerificationHours: DEFAULT_EMAIL_VERIFICATION_HOURS,
			emailChangeHours: DEFAULT_EMAIL_CHANGE_HOURS
		})
	);
}

const OAUTH_ERRORS = new Map<string, () => string>([
	['oauth_cancelled', () => t('signin-oauth-error-oauth-cancelled')],
	['oauth_state_invalid', () => t('signin-oauth-error-oauth-state-invalid')],
	['email_in_use', () => t('signin-oauth-error-email-in-use')],
	['email_required', () => t('signin-oauth-error-email-required')],
	['provider_unavailable', () => t('signin-oauth-error-provider-unavailable')],
	['identity_taken', () => t('signin-oauth-error-identity-taken')],
	['provider_linked', () => t('signin-oauth-error-provider-linked')],
	['account_disabled', () => t('signin-oauth-error-account-disabled')],
	['email_not_verified', () => t('signin-oauth-error-email-not-verified')],
	['email_unverified', () => t('signin-oauth-error-email-unverified')],
	['reauth_required', () => t('signin-oauth-error-reauth-required')],
	['rate_limited', () => t('signin-oauth-error-rate-limited')],
	['not_found', () => t('signin-oauth-error-not-found')],
	['unauthenticated', () => t('signin-oauth-error-unauthenticated')]
]);

export function oauthErrorMessage(code: string | null): string | null {
	if (!code) return null;
	return OAUTH_ERRORS.get(code)?.() ?? t('signin-oauth-error-failed');
}
