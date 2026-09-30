import { FluentBundle, FluentResource, type FluentVariable } from '@fluent/bundle';
import { negotiateLanguages } from '@fluent/langneg';

/** The language every other one falls back to, and the only one bundled with the app. */
export const DEFAULT_LOCALE = 'en';

const STORAGE_KEY = 'locale';

export type Args = Record<string, FluentVariable>;

// The catalog is `/locales/<language>/web/*.ftl`, shared with the server's half next to
// it. The default language ships in the main bundle; every other one is its own chunk,
// fetched only when that language is used, so adding languages costs nobody who does not
// read them.
const bundled = import.meta.glob('../../../../locales/en/web/*.ftl', {
	query: '?raw',
	import: 'default',
	eager: true
}) as Record<string, string>;
const lazy = import.meta.glob(
	['../../../../locales/*/web/*.ftl', '!../../../../locales/en/web/*.ftl'],
	{ query: '?raw', import: 'default' }
) as Record<string, () => Promise<string>>;

const tagOf = (path: string) => /locales\/([^/]+)\/web\//.exec(path)?.[1] ?? '';

export const LOCALES: readonly string[] = [DEFAULT_LOCALE, ...Object.keys(lazy).map(tagOf)].filter(
	(tag, index, all) => tag && all.indexOf(tag) === index
);

/**
 * A language's files, parsed. Throws when files define the same message twice. (The
 * runtime parser skips syntax errors silently; `tests/lib/catalog.test.ts` reports them.)
 */
export function createBundle(tag: string, sources: readonly string[]): FluentBundle {
	// No Unicode isolation marks around values (Fluent's default wraps each one): every
	// language so far is left-to-right, and the marks would end up nested in composed
	// messages, in `aria-label`s and titles, and in copied text. Turn them on
	// (`useIsolating: true`) with the first right-to-left language.
	const bundle = new FluentBundle(tag, { useIsolating: false });
	const problems: string[] = [];
	for (const source of sources) {
		const resource = new FluentResource(source);
		const errors = bundle.addResource(resource);
		problems.push(...errors.map((error) => `${tag}: ${error.message}`));
	}
	if (problems.length)
		throw new Error(`invalid translation catalog:\n  - ${problems.join('\n  - ')}`);
	return bundle;
}

/**
 * The words of the app, in the reader's language.
 *
 * `t(id, args)` reads the current locale, so anything rendered with it (a template, a
 * `$derived`) updates when `use()` switches language. A message the language lacks falls
 * back to English, and one nobody has renders as its id, so a missing translation shows
 * up as an odd label instead of breaking a page. Tests turn `strict` on: there a missing
 * message throws.
 */
export class Translations {
	locale = $state(DEFAULT_LOCALE);
	strict = false;
	#bundles = $state.raw<Record<string, FluentBundle>>({
		[DEFAULT_LOCALE]: createBundle(DEFAULT_LOCALE, Object.values(bundled))
	});

	#locales = [...LOCALES];

	get locales(): readonly string[] {
		return this.#locales;
	}

	t = (id: string, args?: Args): string => {
		const bundles = [this.#bundles[this.locale], this.#bundles[DEFAULT_LOCALE]];
		for (const bundle of bundles) {
			const pattern = bundle?.getMessage(id)?.value;
			if (!bundle || !pattern) continue;
			const errors: Error[] = [];
			const text = bundle.formatPattern(pattern, args, errors);
			if (errors.length && this.strict) throw new Error(`${id}: ${errors[0]?.message}`);
			return text;
		}
		if (this.strict) throw new Error(`missing translation: ${id}`);
		console.warn(`missing translation: ${id}`);
		return id;
	};

	/**
	 * Adds a language from `sources` (Fluent text) without a file in the catalog, so `use(tag)`
	 * finds it. For tests, which need a second language without shipping one.
	 */
	register(tag: string, sources: readonly string[]): void {
		this.#bundles = { ...this.#bundles, [tag]: createBundle(tag, sources) };
		if (!this.#locales.includes(tag)) this.#locales = [...this.#locales, tag];
	}

	negotiate(requested: readonly string[]): string {
		return (
			negotiateLanguages([...requested], [...this.locales], {
				defaultLocale: DEFAULT_LOCALE,
				strategy: 'lookup'
			})[0] ?? DEFAULT_LOCALE
		);
	}

	async use(requested: string): Promise<string> {
		const tag = this.negotiate([requested]);
		if (!this.#bundles[tag]) {
			const files = Object.entries(lazy).filter(([path]) => tagOf(path) === tag);
			const sources = await Promise.all(files.map(([, load]) => load()));
			this.#bundles = { ...this.#bundles, [tag]: createBundle(tag, sources) };
		}
		this.locale = tag;
		if (typeof document !== 'undefined') document.documentElement.lang = tag;
		return tag;
	}

	/**
	 * Picks the language for a visit: the one chosen earlier, else the browser's, else the
	 * default; then loads it. Call before the first render.
	 */
	async init(): Promise<string> {
		let saved: string | null = null;
		try {
			saved = globalThis.localStorage?.getItem(STORAGE_KEY) ?? null;
		} catch {
			// Storage can be blocked; the browser's language is a fine default.
		}
		const preferred = [saved, ...(globalThis.navigator?.languages ?? [])].filter(
			(tag): tag is string => !!tag
		);
		return this.use(this.negotiate(preferred));
	}

	async choose(requested: string): Promise<string> {
		const tag = await this.use(requested);
		try {
			globalThis.localStorage?.setItem(STORAGE_KEY, tag);
		} catch {
			// Not remembered; the switch itself worked.
		}
		return tag;
	}
}

export const i18n = new Translations();

export const t = i18n.t;
