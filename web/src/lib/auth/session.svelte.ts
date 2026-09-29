import type { SignedIn } from '$lib/api';
import type { Permission } from '$lib/types/api';
import { type Action, type Policy, hasPermission } from './permissions';

/**
 * Who is signed in, as reactive state for components.
 *
 * A view of the root layout's `load` data, the one source of truth: the root layout
 * connects it, and signing in, out or changing the profile re-runs that `load`
 * (`invalidate(SESSION)`), so this follows. Guards in `+layout.ts` read the layout data
 * directly, so they run before a page renders.
 */
export class SessionState {
	#source = $state<() => SignedIn | null>(() => null);

	readonly me = $derived(this.#source());
	readonly user = $derived(this.me?.user ?? null);
	readonly isAuthenticated = $derived(this.me !== null);
	readonly needsVerification = $derived(this.me !== null && !this.me.user.emailVerified);

	connect(source: () => SignedIn | null): void {
		this.#source = source;
	}

	has(permission: Permission): boolean {
		return hasPermission(this.me, permission);
	}

	can<R>(policy: Policy<R>, action: Action, resource?: R): boolean {
		return policy(this.me, action, resource);
	}
}

export const session = new SessionState();
