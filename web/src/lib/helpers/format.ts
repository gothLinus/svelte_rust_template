import { type Timestamp, timestampDate } from '@bufbuild/protobuf/wkt';
import { i18n, t } from '$lib/i18n';

interface Formats {
	date: Intl.DateTimeFormat;
	dateTime: Intl.DateTimeFormat;
	relative: Intl.RelativeTimeFormat;
}

const formats = new Map<string, Formats>();

function current(): Formats {
	const locale = i18n.locale;
	let found = formats.get(locale);
	if (!found) {
		found = {
			date: new Intl.DateTimeFormat(locale, { dateStyle: 'medium' }),
			dateTime: new Intl.DateTimeFormat(locale, { dateStyle: 'medium', timeStyle: 'short' }),
			relative: new Intl.RelativeTimeFormat(locale, { numeric: 'auto' })
		};
		formats.set(locale, found);
	}
	return found;
}

const UNITS: [Intl.RelativeTimeFormatUnit, number][] = [
	['year', 365 * 24 * 60 * 60],
	['month', 30 * 24 * 60 * 60],
	['week', 7 * 24 * 60 * 60],
	['day', 24 * 60 * 60],
	['hour', 60 * 60],
	['minute', 60]
];

/**
 * A timestamp from the API as a date in the current language, e.g. "Sep 25, 2026".
 * Messages leave a timestamp field unset only where the schema says it may be absent;
 * that formats as "".
 */
export function formatDate(timestamp: Timestamp | undefined): string {
	return timestamp ? current().date.format(timestampDate(timestamp)) : '';
}

export function formatDateTime(timestamp: Timestamp | undefined): string {
	return timestamp ? current().dateTime.format(timestampDate(timestamp)) : '';
}

export function formatRelative(timestamp: Timestamp | undefined, now = Date.now()): string {
	if (!timestamp) return '';
	const { relative } = current();
	const seconds = (timestampDate(timestamp).getTime() - now) / 1000;
	for (const [unit, size] of UNITS) {
		if (Math.abs(seconds) >= size) return relative.format(Math.round(seconds / size), unit);
	}
	return relative.format(0, 'second');
}

export function initials(name: string): string {
	const parts = name.trim().split(/\s+/).filter(Boolean);
	const letters = parts.length > 1 ? [parts[0] ?? '', parts.at(-1) ?? ''] : parts;
	return letters.map((part) => [...part][0]?.toUpperCase() ?? '').join('') || '?';
}

/**
 * A server message as a sentence of its own: capitalised and ending in a full stop. Problem
 * details are fragments ("this link is invalid or has expired") meant to follow a field name.
 */
export function asSentence(text: string): string {
	const trimmed = text.trim();
	if (!trimmed) return trimmed;
	const capitalised = trimmed.charAt(0).toLocaleUpperCase(i18n.locale) + trimmed.slice(1);
	return /[.!?…]$/.test(capitalised) ? capitalised : `${capitalised}.`;
}

export function excerpt(text: string, max = 140): string {
	const line = text.trim().split('\n')[0] ?? '';
	const chars = [...line];
	return chars.length > max
		? `${chars
				.slice(0, max - 1)
				.join('')
				.trimEnd()}…`
		: line;
}

const BROWSERS: [RegExp, string][] = [
	[/Edg\//, 'Edge'],
	[/OPR\/|Opera/, 'Opera'],
	[/Firefox\//, 'Firefox'],
	[/Chrome\/|CriOS\//, 'Chrome'],
	[/Safari\//, 'Safari']
];

const SYSTEMS: [RegExp, string][] = [
	[/iPhone|iPad|iPod/, 'iOS'],
	[/Android/, 'Android'],
	[/Mac OS X|Macintosh/, 'macOS'],
	[/Windows/, 'Windows'],
	[/CrOS/, 'ChromeOS'],
	[/Linux/, 'Linux']
];

export function describeUserAgent(userAgent: string | null | undefined): string {
	if (!userAgent) return t('session-device-unknown');
	const browser = BROWSERS.find(([pattern]) => pattern.test(userAgent))?.[1];
	const system = SYSTEMS.find(([pattern]) => pattern.test(userAgent))?.[1];
	if (browser && system) return t('session-device', { browser, system });
	return browser ?? system ?? userAgent.split(/[\s/]/)[0] ?? t('session-device-unknown');
}
