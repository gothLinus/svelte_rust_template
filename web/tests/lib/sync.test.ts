import { beforeEach, describe, expect, it, vi } from 'vitest';
import { me } from '../helpers';

vi.mock('$app/navigation', () => ({ goto: vi.fn(), invalidate: vi.fn(), invalidateAll: vi.fn() }));

const { EVERYTHING, RECHECK_INTERVAL_MS, SESSION, TabSync, sessionChanged } =
	await import('$lib/auth');

class FakeChannel extends EventTarget {
	peer: FakeChannel | null = null;
	postMessage(data: unknown) {
		this.peer?.dispatchEvent(new MessageEvent('message', { data }));
	}
}

class FakePage extends EventTarget {
	visibilityState: DocumentVisibilityState = 'visible';
	show() {
		this.visibilityState = 'visible';
		this.dispatchEvent(new Event('visibilitychange'));
	}
	hide() {
		this.visibilityState = 'hidden';
		this.dispatchEvent(new Event('visibilitychange'));
	}
}

let clock = 0;

function tab(channel: FakeChannel | null, changed = true) {
	const invalidate = vi.fn(async () => {});
	const invalidateAll = vi.fn(async () => {});
	const changes = vi.fn(async () => changed);
	const sync = new TabSync({
		channel: () => channel,
		invalidate,
		invalidateAll,
		sessionChanged: changes,
		now: () => clock
	});
	return { sync, invalidate, invalidateAll, changes };
}

function twoTabs() {
	const a = new FakeChannel();
	const b = new FakeChannel();
	a.peer = b;
	b.peer = a;
	return [tab(a), tab(b)] as const;
}

beforeEach(() => {
	clock = 0;
});

describe('syncing tabs', () => {
	it('re-runs a changed key here and in the other tabs', async () => {
		const [here, other] = twoTabs();
		other.sync.listen(new FakePage() as unknown as Document);

		await here.sync.refresh(SESSION);

		expect(here.invalidate).toHaveBeenCalledWith(SESSION);
		expect(other.invalidate).toHaveBeenCalledWith(SESSION);
		expect(here.invalidate).toHaveBeenCalledOnce();
	});

	it('re-runs everything in the other tabs after signing in or out', () => {
		const [here, other] = twoTabs();
		other.sync.listen(new FakePage() as unknown as Document);

		here.sync.announce(EVERYTHING);

		expect(other.invalidateAll).toHaveBeenCalledOnce();
		expect(here.invalidateAll).not.toHaveBeenCalled();
	});

	it('stops listening', async () => {
		const [here, other] = twoTabs();
		const stop = other.sync.listen(new FakePage() as unknown as Document);
		stop();

		await here.sync.refresh(SESSION);
		expect(other.invalidate).not.toHaveBeenCalled();
	});

	it('works alone where tabs cannot talk', async () => {
		const alone = tab(null);
		alone.sync.listen(new FakePage() as unknown as Document);
		await alone.sync.refresh('app:notes');
		expect(alone.invalidate).toHaveBeenCalledWith('app:notes');
	});

	it('re-checks the session when the tab comes back into view, not too often', async () => {
		const { sync, invalidate, changes } = tab(null);
		const page = new FakePage();
		sync.listen(page as unknown as Document);

		page.hide();
		page.show();
		expect(changes).not.toHaveBeenCalled();

		clock = RECHECK_INTERVAL_MS;
		page.hide();
		expect(changes).not.toHaveBeenCalled();
		page.show();
		await vi.waitFor(() => expect(invalidate).toHaveBeenCalledExactlyOnceWith(SESSION));

		clock += RECHECK_INTERVAL_MS - 1;
		page.show();
		expect(changes).toHaveBeenCalledOnce();
	});

	it('re-runs nothing when the session did not change', async () => {
		const { sync, invalidate, changes } = tab(null, false);
		const page = new FakePage();
		sync.listen(page as unknown as Document);

		clock = RECHECK_INTERVAL_MS;
		page.show();
		await vi.waitFor(() => expect(changes).toHaveBeenCalledOnce());
		expect(invalidate).not.toHaveBeenCalled();
	});

	it('counts a session refresh from another tab as a re-check', () => {
		const [here, other] = twoTabs();
		const page = new FakePage();
		other.sync.listen(page as unknown as Document);

		clock = RECHECK_INTERVAL_MS;
		here.sync.announce(SESSION);
		page.show();
		expect(other.changes).not.toHaveBeenCalled();
		expect(other.invalidate).toHaveBeenCalledOnce();
	});
});

describe('whether the session changed', () => {
	it('compares what the server says with what the tab shows', async () => {
		const shown = me();
		expect(await sessionChanged(shown, async () => me())).toBe(false);

		const verified = me();
		verified.user.emailVerified = !shown.user.emailVerified;
		expect(await sessionChanged(shown, async () => verified)).toBe(true);
	});

	it('notices a session that ended elsewhere', async () => {
		expect(await sessionChanged(me(), async () => null)).toBe(true);
	});

	it('asks nothing when signed out, and ignores failures', async () => {
		const fetchMe = vi.fn(async () => me());
		expect(await sessionChanged(null, fetchMe)).toBe(false);
		expect(fetchMe).not.toHaveBeenCalled();

		expect(
			await sessionChanged(me(), async () => {
				throw new Error('offline');
			})
		).toBe(false);
	});
});
