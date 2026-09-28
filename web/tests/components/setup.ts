import '@testing-library/jest-dom/vitest';
import { afterEach, vi } from 'vitest';
import { i18n } from '$lib/i18n';

/**
 * Components render in jsdom without a SvelteKit app around them, so `$app/*` is replaced
 * by the fakes in `./fake-app.svelte.ts`. Tests change `page` and inspect the navigation spies there.
 */
vi.mock('$app/state', async () => ({ page: (await import('./fake-app.svelte')).page }));
vi.mock('$app/navigation', async () => (await import('./fake-app.svelte')).navigation);
vi.mock('$app/paths', () => ({ resolve: (path: string) => path, asset: (path: string) => path }));

// jsdom lacks these browser APIs, which bits-ui and mode-watcher use.
Object.defineProperty(window, 'matchMedia', {
	configurable: true,
	value: (query: string) => ({
		matches: false,
		media: query,
		onchange: null,
		addEventListener: () => {},
		removeEventListener: () => {},
		addListener: () => {},
		removeListener: () => {},
		dispatchEvent: () => false
	})
});
class ResizeObserverStub {
	observe() {}
	unobserve() {}
	disconnect() {}
}
globalThis.ResizeObserver ??= ResizeObserverStub;
Element.prototype.scrollIntoView ??= () => {};
Element.prototype.hasPointerCapture ??= () => false;
Element.prototype.releasePointerCapture ??= () => {};

afterEach(async () => {
	const { reset } = await import('./fake-app.svelte');
	reset();
	vi.unstubAllGlobals();
	// A test that switched language must not leak it into the next one.
	await i18n.use('en');
	// An open bits-ui dialog locks the page (`pointer-events: none` on <body>).
	document.body.removeAttribute('style');
});
