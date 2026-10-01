import { fireEvent, render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { session } from '$lib/auth';
import LanguageMenu from '$lib/components/language-menu.svelte';
import LanguageCard from '$lib/components/profile/language-card.svelte';
import { chooseLanguage, followAccountLanguage, languageName } from '$lib/helpers/language';
import { LOCALES, i18n, t } from '$lib/i18n';
import { MeSchema, SetLocaleRequestSchema } from '$lib/types/api';
import { callOf, me, mockFetch, reply, sent, useEnglish } from '../helpers';
import { navigation } from './fake-app.svelte';

const DEUTSCH = 'Deutsch';

beforeEach(() => session.connect(() => null));

afterEach(useEnglish);

describe('the catalog', () => {
	it('ships German next to English', () => {
		expect(LOCALES).toEqual(expect.arrayContaining(['en', 'de']));
	});

	it('words the app in German once chosen', async () => {
		const english = t('nav-dashboard');
		await i18n.use('de');
		expect(t('nav-dashboard')).not.toBe(english);
		expect(document.documentElement.lang).toBe('de');
	});
});

describe('languageName', () => {
	it('names each language in itself', () => {
		expect(languageName('de')).toBe(DEUTSCH);
		expect(languageName('en')).toBe('English');
	});

	it('falls back to the tag for one it cannot name', () => {
		expect(languageName('not a tag')).toBe('not a tag');
	});
});

describe('chooseLanguage', () => {
	it('only switches the app while signed out', async () => {
		const fetchFn = mockFetch(reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);

		await expect(chooseLanguage('de')).resolves.toBe(true);

		expect(i18n.locale).toBe('de');
		expect(fetchFn).not.toHaveBeenCalled();
	});

	it('keeps the choice on the account while signed in', async () => {
		session.connect(() => me());
		const fetchFn = mockFetch(reply(MeSchema, me([], undefined, { locale: 'de' })));
		vi.stubGlobal('fetch', fetchFn);

		await chooseLanguage('de');

		expect(callOf(fetchFn)[0]).toBe('/api/v1/me/locale');
		expect(callOf(fetchFn)[1].method).toBe('PUT');
		expect(sent(fetchFn, SetLocaleRequestSchema).locale).toBe('de');
		expect(navigation.invalidate).toHaveBeenCalled();
	});

	it('does not ask the server when the account has the language already', async () => {
		session.connect(() => me([], undefined, { locale: 'de' }));
		const fetchFn = mockFetch(reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);

		await chooseLanguage('de');

		expect(fetchFn).not.toHaveBeenCalled();
	});
});

describe('followAccountLanguage', () => {
	it('switches to the language of the account', async () => {
		await followAccountLanguage('de');
		expect(i18n.locale).toBe('de');
	});

	it('ignores no language, the current one and one the app lacks', async () => {
		for (const locale of [undefined, 'en', 'xx']) {
			await followAccountLanguage(locale);
			expect(i18n.locale).toBe('en');
		}
	});
});

describe('language menu', () => {
	it('lists every language by its own name and switches', async () => {
		render(LanguageMenu);

		await fireEvent.pointerDown(
			screen.getByRole('button', { name: `${t('language-label')}: English` }),
			{ button: 0, pointerType: 'mouse' }
		);
		await fireEvent.click(await screen.findByRole('menuitemradio', { name: DEUTSCH }));

		await vi.waitFor(() => expect(i18n.locale).toBe('de'));
	});
});

describe('language card', () => {
	it('marks the current language and saves another', async () => {
		const user = userEvent.setup();
		session.connect(() => me());
		vi.stubGlobal('fetch', mockFetch(reply(MeSchema, me([], undefined, { locale: 'de' }))));
		render(LanguageCard);

		expect(screen.getByRole('radio', { name: 'English' })).toHaveAttribute('aria-checked', 'true');
		await user.click(screen.getByRole('radio', { name: DEUTSCH }));

		await vi.waitFor(() => expect(i18n.locale).toBe('de'));
		expect(screen.getByRole('radio', { name: DEUTSCH })).toHaveAttribute('aria-checked', 'true');
	});
});
