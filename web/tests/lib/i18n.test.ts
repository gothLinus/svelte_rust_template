import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { DEFAULT_LOCALE, LOCALES, createBundle, i18n, t } from '$lib/i18n';
import { GERMAN, useEnglish, useGerman } from '../helpers';

// Plural categories differ by language: Russian has one, few, many and other.
const RUSSIAN = `
notes-count = { $count ->
    [one] { $count } заметка
    [few] { $count } заметки
    [many] { $count } заметок
   *[other] { $count } заметки
  }
`;

function fakeStorage(saved: Record<string, string> = {}) {
	const data = new Map(Object.entries(saved));
	return {
		data,
		blocked: false,
		getItem(key: string) {
			if (this.blocked) throw new DOMException('blocked', 'SecurityError');
			return data.get(key) ?? null;
		},
		setItem(key: string, value: string) {
			if (this.blocked) throw new DOMException('blocked', 'SecurityError');
			data.set(key, value);
		}
	};
}

beforeEach(() => {
	i18n.register('de', [GERMAN]);
	i18n.register('ru', [RUSSIAN]);
});

afterEach(async () => {
	vi.unstubAllGlobals();
	i18n.strict = true;
	await useEnglish();
});

describe('t', () => {
	it('reads the message of the language in use', async () => {
		expect(t('common-cancel')).toBe('Cancel');
		await useGerman();
		expect(t('error-network')).toBe('Der Server ist nicht erreichbar.');
	});

	it('falls back to English for a message the language lacks', async () => {
		await useGerman();
		expect(t('common-cancel')).toBe('Cancel');
		expect(i18n.locale).toBe('de');
	});

	it('fills in placeholders', () => {
		expect(t('security-linked-unlinked', { provider: 'GitHub' })).toBe('GitHub unlinked.');
	});

	it('chooses a plural by the language rules', async () => {
		// English: the singular reads differently from every other number.
		expect(t('validation-too-long', { max: 1 })).toBe('must be at most 1 character');
		expect(t('validation-too-long', { max: 5 })).toBe('must be at most 5 characters');

		i18n.strict = false;
		await i18n.use('ru');
		const words = [1, 2, 5, 21].map((count) => t('notes-count', { count }));
		expect(words).toEqual(['1 заметка', '2 заметки', '5 заметок', '21 заметка']);
	});

	it('throws for a missing message in strict mode', () => {
		expect(() => t('no-such-message')).toThrow('missing translation: no-such-message');
	});

	it('throws for a missing placeholder in strict mode', () => {
		expect(() => t('validation-too-short')).toThrow('validation-too-short');
	});

	it('shows the id of a missing message otherwise, and says so', () => {
		i18n.strict = false;
		const warn = vi.spyOn(console, 'warn').mockImplementation(() => {});
		expect(t('no-such-message')).toBe('no-such-message');
		expect(warn).toHaveBeenCalledWith('missing translation: no-such-message');
		warn.mockRestore();
	});
});

describe('negotiate', () => {
	it('picks the best language the catalog has', () => {
		expect(i18n.negotiate(['de-AT', 'en'])).toBe('de');
		expect(i18n.negotiate(['fr', 'ru-RU', 'en'])).toBe('ru');
		expect(i18n.negotiate(['en-GB'])).toBe('en');
	});

	it('falls back to the default language', () => {
		expect(i18n.negotiate(['ja', 'fr'])).toBe(DEFAULT_LOCALE);
		expect(i18n.negotiate([])).toBe(DEFAULT_LOCALE);
	});

	it('knows the catalog languages and the registered ones', () => {
		expect(LOCALES).toContain('en');
		expect(i18n.locales).toEqual(expect.arrayContaining([...LOCALES, 'de', 'ru']));
	});
});

describe('use', () => {
	it('switches to the negotiated language and answers with it', async () => {
		await expect(i18n.use('de-CH')).resolves.toBe('de');
		expect(i18n.locale).toBe('de');
		await expect(i18n.use('xx')).resolves.toBe('en');
		expect(i18n.locale).toBe('en');
	});

	it('registering twice replaces the messages, not the language list', async () => {
		const before = i18n.locales.length;
		i18n.register('de', ['error-network = Nicht erreichbar.']);
		expect(i18n.locales).toHaveLength(before);
		await i18n.use('de');
		expect(t('error-network')).toBe('Nicht erreichbar.');
	});
});

describe('choose and init', () => {
	it('remembers the chosen language, and init restores it', async () => {
		const storage = fakeStorage();
		vi.stubGlobal('localStorage', storage);

		await expect(i18n.choose('de')).resolves.toBe('de');
		expect(storage.data.get('locale')).toBe('de');

		await i18n.use('en');
		await expect(i18n.init()).resolves.toBe('de');
		expect(i18n.locale).toBe('de');
	});

	it('remembers what was negotiated, not what was asked for', async () => {
		const storage = fakeStorage();
		vi.stubGlobal('localStorage', storage);
		await i18n.choose('ru-RU');
		expect(storage.data.get('locale')).toBe('ru');
	});

	it('prefers the saved language over the browser’s', async () => {
		vi.stubGlobal('localStorage', fakeStorage({ locale: 'ru' }));
		vi.stubGlobal('navigator', { languages: ['de-DE', 'en'] });
		await expect(i18n.init()).resolves.toBe('ru');
	});

	it('follows the browser’s languages without a saved choice', async () => {
		vi.stubGlobal('localStorage', fakeStorage());
		vi.stubGlobal('navigator', { languages: ['fr-FR', 'de-DE', 'en'] });
		await expect(i18n.init()).resolves.toBe('de');
	});

	it('uses the default language without any preference', async () => {
		vi.stubGlobal('localStorage', fakeStorage());
		vi.stubGlobal('navigator', { languages: [] });
		await expect(i18n.init()).resolves.toBe('en');
	});

	it('works with storage blocked', async () => {
		const storage = fakeStorage();
		storage.blocked = true;
		vi.stubGlobal('localStorage', storage);
		vi.stubGlobal('navigator', { languages: ['de'] });

		await expect(i18n.init()).resolves.toBe('de');
		await expect(i18n.choose('ru')).resolves.toBe('ru');
		expect(i18n.locale).toBe('ru');
	});
});

describe('createBundle', () => {
	it('rejects a message defined twice', () => {
		expect(() => createBundle('en', ['a = 1', 'a = 2'])).toThrow('invalid translation catalog');
	});

	it('does not wrap values in direction marks', () => {
		const bundle = createBundle('en', ['hello = Hello, { $name }!']);
		const pattern = bundle.getMessage('hello')?.value;
		expect(bundle.formatPattern(pattern!, { name: 'Ada' })).toBe('Hello, Ada!');
	});
});
