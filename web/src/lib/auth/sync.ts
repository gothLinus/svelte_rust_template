import { equals } from '@bufbuild/protobuf';
import { invalidate, invalidateAll } from '$app/navigation';
import { type SignedIn, api } from '$lib/api';
import { MeSchema } from '$lib/types/api';
import { SESSION } from './guards';
import { session } from './session.svelte';

/**
 * Keeps every tab of this browser showing the same thing. Tabs share the session cookie
 * but not their `load` data, so a change made in one tab (the email verified from a
 * mailed link, a passkey added, signing out) would stay invisible in the others until a
 * reload. The tab that made the change tells the others which `depends()` key to re-run,
 * over a `BroadcastChannel`.
 *
 * Changes made on another device (the link opened on a phone, a session revoked there)
 * cannot be announced: a signed-in tab coming back into view asks the server who is signed
 * in, one small request, and re-runs its `load` functions only when the answer changed.
 */

export const EVERYTHING = '*';

export const RECHECK_INTERVAL_MS = 60_000;

const CHANNEL = 'app:sync';

type Channel = Pick<BroadcastChannel, 'postMessage' | 'addEventListener' | 'removeEventListener'>;
type Page = Pick<Document, 'visibilityState' | 'addEventListener' | 'removeEventListener'>;

export interface TabSyncEffects {
	channel: () => Channel | null;
	invalidate: (key: string) => Promise<void>;
	invalidateAll: () => Promise<void>;
	sessionChanged: () => Promise<boolean>;
	now?: () => number;
}

export class TabSync {
	#effects: TabSyncEffects;
	#now: () => number;
	#channel: Channel | null | undefined;
	#checkedAt: number;

	constructor(effects: TabSyncEffects) {
		this.#effects = effects;
		this.#now = effects.now ?? Date.now;
		this.#checkedAt = this.#now();
	}

	async refresh(key: string): Promise<void> {
		this.announce(key);
		await this.#apply(key);
	}

	/**
	 * Tells the other tabs to re-run `key`, when this one does so itself, e.g. with
	 * `goto(..., { invalidateAll: true })` after signing in.
	 */
	announce(key: string): void {
		this.#open()?.postMessage(key);
	}

	/**
	 * Follows the other tabs' announcements, and re-checks the session when `page` comes
	 * back into view. The root layout calls this once; returns a function that stops it.
	 */
	listen(page: Page = document): () => void {
		const channel = this.#open();
		const onMessage = (event: MessageEvent) => {
			if (typeof event.data === 'string') void this.#apply(event.data);
		};
		const onVisible = async () => {
			if (page.visibilityState !== 'visible') return;
			if (this.#now() - this.#checkedAt < RECHECK_INTERVAL_MS) return;
			this.#checkedAt = this.#now();
			if (await this.#effects.sessionChanged()) await this.#effects.invalidate(SESSION);
		};
		channel?.addEventListener('message', onMessage);
		page.addEventListener('visibilitychange', onVisible);
		return () => {
			channel?.removeEventListener('message', onMessage);
			page.removeEventListener('visibilitychange', onVisible);
		};
	}

	#apply(key: string): Promise<void> {
		if (key === SESSION || key === EVERYTHING) this.#checkedAt = this.#now();
		return key === EVERYTHING ? this.#effects.invalidateAll() : this.#effects.invalidate(key);
	}

	#open(): Channel | null {
		if (this.#channel === undefined) this.#channel = this.#effects.channel();
		return this.#channel;
	}
}

/**
 * Whether the signed-in user the server reports differs from `current`, what this tab
 * shows: the email verified, roles changed, the session ended. A signed-out tab has nothing
 * to re-check, since another device cannot sign this browser in, and a failed request
 * changes nothing: the next `load` will show the error.
 */
export async function sessionChanged(
	current: SignedIn | null,
	fetchMe: () => Promise<SignedIn | null>
): Promise<boolean> {
	if (!current) return false;
	try {
		const fresh = await fetchMe();
		return fresh === null || !equals(MeSchema, fresh, current);
	} catch {
		return false;
	}
}

export const tabSync = new TabSync({
	channel: () => (typeof BroadcastChannel === 'undefined' ? null : new BroadcastChannel(CHANNEL)),
	invalidate: (key) => invalidate(key),
	invalidateAll: () => invalidateAll(),
	sessionChanged: () => sessionChanged(session.me, () => api.me.current())
});
