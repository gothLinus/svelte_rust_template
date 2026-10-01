import { isHttpError } from '@sveltejs/kit';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '$lib/api';
import {
	asSentence,
	describeUserAgent,
	excerpt,
	formatDate,
	formatDateTime,
	formatRelative,
	initials
} from '$lib/helpers/format';
import { fromApi } from '$lib/helpers/load';
import { t } from '$lib/i18n';
import { MAX_TABS, activeItem, navigationFor, tabBarItems } from '$lib/helpers/navigation';
import { CursorTrail } from '$lib/helpers/pagination';
import { captureTokenFromHash, takeToken } from '$lib/helpers/token';
import { withQuery } from '$lib/helpers/url';
import { Permission } from '$lib/types/api';
import { site } from '$lib/helpers/site.svelte';
import { me, ts, useEnglish, useGerman } from '../helpers';

describe('format', () => {
	it('makes a server message a sentence of its own', () => {
		expect(asSentence('this link is invalid or has expired')).toBe(
			'This link is invalid or has expired.'
		);
		expect(asSentence('  Already a sentence!  ')).toBe('Already a sentence!');
		expect(asSentence('')).toBe('');
	});

	const now = Date.parse('2026-06-15T12:00:00Z');

	afterEach(useEnglish);

	it('formats relative times', () => {
		expect(formatRelative(ts('2026-06-15T11:59:30Z'), now)).toMatch(/now/);
		expect(formatRelative(ts('2026-06-15T11:55:00Z'), now)).toBe('5 minutes ago');
		expect(formatRelative(ts('2026-06-14T12:00:00Z'), now)).toBe('yesterday');
		expect(formatRelative(ts('2026-06-29T12:00:00Z'), now)).toBe('in 2 weeks');
	});

	it('formats dates', () => {
		expect(formatDate(ts('2026-06-15T12:00:00Z'))).toMatch(/2026/);
		expect(formatDateTime(ts('2026-06-15T12:00:00Z'))).toMatch(/2026/);
	});

	it('formats dates and relative times in the language in use', async () => {
		const date = ts('2026-06-15T12:00:00Z');
		expect(formatDate(date)).toBe('Jun 15, 2026');

		await useGerman();
		expect(formatDate(date)).toBe('15.06.2026');
		expect(formatDateTime(date)).toMatch(/^15\.06\.2026, \d\d:\d\d$/);
		expect(formatRelative(ts('2026-06-15T11:55:00Z'), now)).toBe('vor 5 Minuten');
		expect(formatRelative(ts('2026-06-14T12:00:00Z'), now)).toBe('gestern');

		await useEnglish();
		expect(formatDate(date)).toBe('Jun 15, 2026');
		expect(formatRelative(ts('2026-06-14T12:00:00Z'), now)).toBe('yesterday');
	});

	it('formats a missing timestamp as nothing', () => {
		expect(formatDate(undefined)).toBe('');
		expect(formatDateTime(undefined)).toBe('');
		expect(formatRelative(undefined)).toBe('');
	});

	it('makes initials', () => {
		expect(initials('Alice Pleasance Liddell')).toBe('AL');
		expect(initials('  bob ')).toBe('B');
		expect(initials('')).toBe('?');
	});

	it('shortens text to its first line', () => {
		expect(excerpt('first line\nsecond')).toBe('first line');
		expect(excerpt('abcdef', 4)).toBe('abc…');
		expect(excerpt('  ')).toBe('');
	});

	it('describes user agents', () => {
		expect(
			describeUserAgent('Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) Gecko/20100101 Firefox/130.0')
		).toBe(t('session-device', { browser: 'Firefox', system: 'macOS' }));
		expect(describeUserAgent('curl/8.7.1')).toBe('curl');
		expect(describeUserAgent(null)).toBe(t('session-device-unknown'));
		expect(describeUserAgent('Mozilla/5.0 (X11; Linux x86_64)')).toBe('Linux');
	});

	it('words device labels in the language in use', async () => {
		await useGerman();
		expect(describeUserAgent('Mozilla/5.0 (Windows NT 10.0) Firefox/130.0')).toBe(
			'Firefox auf Windows'
		);
		expect(describeUserAgent(undefined)).toBe('Unbekanntes Gerät');
	});
});

describe('navigation', () => {
	afterEach(useEnglish);

	it('shows items by permission', () => {
		const titles = (permissions: Parameters<typeof me>[0]) =>
			navigationFor(me(permissions)).flatMap((group) => group.items.map((item) => item.title));

		const [dashboard, notes, profile, security, users] = [
			t('nav-dashboard'),
			t('notes-nav'),
			t('nav-profile'),
			t('nav-security'),
			t('nav-users')
		];
		expect(titles([Permission.NOTES_READ])).toEqual([dashboard, notes, profile, security]);
		expect(titles([])).toEqual([dashboard, profile, security]);
		expect(titles([Permission.NOTES_READ, Permission.USERS_READ])).toContain(users);
	});

	it('finds the active item, including sub-pages', () => {
		expect(activeItem('/notes')?.item.title).toBe(t('notes-nav'));
		expect(activeItem('/admin/users/123')?.group.label).toBe(t('nav-administration'));
		expect(activeItem('/nowhere')).toBeUndefined();
		expect(activeItem('/notesx')).toBeUndefined();
	});

	it('names pages and groups in the language in use', async () => {
		await useGerman();
		const groups = navigationFor(me());
		expect(groups[0]?.label).toBe('Arbeitsbereich');
		expect(groups[0]?.items[0]?.title).toBe('Übersicht');
		expect(groups[0]?.items[1]?.title).toBe(t('notes-nav'));
		expect(activeItem('/dashboard')?.item.title).toBe('Übersicht');
		expect(site.description).toBe('Ein Starter mit Konten.');

		await useEnglish();
		expect(activeItem('/dashboard')?.item.title).toBe('Dashboard');
	});
});

describe('tabBarItems', () => {
	const items = (count: number) =>
		navigationFor(me([Permission.NOTES_READ, Permission.USERS_READ]))
			.flatMap((group) => group.items)
			.slice(0, count);

	it('shows every item that fits', () => {
		expect(tabBarItems(items(MAX_TABS))).toEqual({ tabs: items(MAX_TABS), more: [] });
	});

	it('moves the rest into "More", keeping the bar at its size', () => {
		const all = items(5);
		const { tabs, more } = tabBarItems(all);
		expect(tabs).toHaveLength(MAX_TABS - 1);
		expect([...tabs, ...more]).toEqual(all);
	});
});

describe('CursorTrail', () => {
	it('walks back through visited pages', () => {
		const trail = new CursorTrail();
		expect(trail.hasPrevious(null)).toBe(false);

		expect(trail.next(null, 'c1')).toBe('c1');
		expect(trail.next('c1', 'c2')).toBe('c2');
		expect(trail.hasPrevious('c2')).toBe(true);
		expect(trail.previous('c2')).toBe('c1');
		expect(trail.previous('c1')).toBeNull();
		expect(trail.previous(null)).toBeNull();
	});

	it('depends only on the page in the URL, so Back and Forward stay right', () => {
		const trail = new CursorTrail();
		trail.next(null, 'c1');
		trail.next('c1', 'c2');

		expect(trail.hasPrevious(null)).toBe(false);
		expect(trail.previous('c1')).toBeNull();
		expect(trail.previous('c2')).toBe('c1');
	});

	it('goes to the first page from a page it never saw', () => {
		const trail = new CursorTrail();
		expect(trail.hasPrevious('c5')).toBe(true);
		expect(trail.previous('c5')).toBeNull();

		trail.next('c5', 'c6');
		trail.reset();
		expect(trail.previous('c6')).toBeNull();
	});
});

describe('withQuery', () => {
	it('sets and removes parameters', () => {
		const url = new URL('http://app.test/notes?scope=all&after=c1');
		expect(withQuery(url, { after: 'c2' })).toBe('/notes?scope=all&after=c2');
		expect(withQuery(url, { after: null })).toBe('/notes?scope=all');
		expect(withQuery(url, { after: null, scope: '' })).toBe('/notes');
	});
});

describe('captureTokenFromHash', () => {
	it('takes the token out of the fragment, once, for its own page', () => {
		const replace = vi.fn();
		captureTokenFromHash(new URL('http://app.test/reset-password?x=1#token=abc'), replace);

		expect(replace).toHaveBeenCalledWith('/reset-password?x=1');
		expect(takeToken('/verify-email')).toBeNull();

		captureTokenFromHash(new URL('http://app.test/reset-password#token=abc'), vi.fn());
		expect(takeToken('/reset-password')).toBe('abc');
		expect(takeToken('/reset-password')).toBeNull();
	});

	it('leaves other fragments alone', () => {
		const replace = vi.fn();
		captureTokenFromHash(new URL('http://app.test/docs#section'), replace);
		captureTokenFromHash(new URL('http://app.test/verify-email'), replace);

		expect(replace).not.toHaveBeenCalled();
		expect(takeToken('/verify-email')).toBeNull();
	});
});

describe('fromApi', () => {
	it('passes results through', async () => {
		await expect(fromApi(async () => 1)).resolves.toBe(1);
	});

	it('turns API errors into error pages', async () => {
		const error = await fromApi(async () => {
			throw new ApiError(404, 'not_found', 'not found');
		}).catch((e: unknown) => e);
		expect(isHttpError(error, 404)).toBe(true);

		const offline = await fromApi(async () => {
			throw ApiError.network();
		}).catch((e: unknown) => e);
		expect(isHttpError(offline, 503)).toBe(true);
	});

	it('rethrows everything else', async () => {
		await expect(
			fromApi(async () => {
				throw new TypeError('bug');
			})
		).rejects.toBeInstanceOf(TypeError);
	});
});
