import { goto } from '$app/navigation';
import type { LoginResult } from '$lib/api';
import { safeRedirect } from './guards';
import { EVERYTHING, tabSync } from './sync';

export function mfaUrl(redirectTo: string | null | undefined): string {
	const target = safeRedirect(redirectTo);
	return `/login/mfa?redirectTo=${encodeURIComponent(target)}`;
}

/**
 * Where a first sign-in step leads: into the app, or to the second step. Every sign-in
 * page ends with this.
 */
export async function finishSignIn(
	result: LoginResult,
	redirectTo: string | null | undefined
): Promise<void> {
	if (result.kind === 'mfa') {
		// `mfaUrl` builds a same-site path.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		await goto(mfaUrl(redirectTo));
		return;
	}
	// Re-running every `load` picks up the new session, the root layout's included, here
	// and in the other tabs: one waiting on a sign-in link or on the login page moves on.
	tabSync.announce(EVERYTHING);
	// `safeRedirect` only lets same-site paths through.
	// eslint-disable-next-line svelte/no-navigation-without-resolve
	await goto(safeRedirect(redirectTo), { invalidateAll: true });
}
