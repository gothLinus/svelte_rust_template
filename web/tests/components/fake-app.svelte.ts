import { vi } from 'vitest';
import { captureTokenFromHash } from '$lib/helpers/token';

class FakePage {
	url = $state(new URL('http://app.test/'));
	state = $state({});
	data = $state<Record<string, unknown>>({});
	error = $state<{ message: string } | null>(null);
	status = $state(200);
}

export const page = new FakePage();

export const navigation = {
	goto: vi.fn(async () => {}),
	invalidate: vi.fn(async () => {}),
	invalidateAll: vi.fn(async () => {}),
	replaceState: vi.fn(),
	pushState: vi.fn()
};

/**
 * Opens `path` (with its query and fragment) as the current page, loaded fresh: an emailed
 * link's token is captured the way the client `init` hook does.
 */
export function visit(path: string): void {
	// Replaced whole, like SvelteKit's `page.url`, so `$state` is enough.
	// eslint-disable-next-line svelte/prefer-svelte-reactivity
	page.url = new URL(path, 'http://app.test');
	captureTokenFromHash(page.url, (scrubbed) => {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity
		page.url = new URL(scrubbed, 'http://app.test');
	});
}

export function reset(): void {
	visit('/');
	page.state = {};
	page.data = {};
	page.error = null;
	page.status = 200;
	for (const spy of Object.values(navigation)) spy.mockClear();
}
