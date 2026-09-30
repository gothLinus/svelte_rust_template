import { readdirSync, readFileSync, statSync } from 'node:fs';
import { join, relative } from 'node:path';
import {
	parse,
	type Entry,
	type Pattern,
	type Expression,
	type InlineExpression
} from '@fluent/syntax';
import { describe, expect, it } from 'vitest';
import { DEFAULT_LOCALE, LOCALES, createBundle, i18n } from '$lib/i18n';

// The guard rails of the translation catalog (`/locales/<language>/web/*.ftl`): the files
// parse, every language says the same things as English, and the code and the catalog
// agree on which messages exist.

const ROOT = join(import.meta.dirname, '../../../locales');
const SRC = join(import.meta.dirname, '../../src');

const languages = readdirSync(ROOT).filter((name) => statSync(join(ROOT, name)).isDirectory());

function sources(language: string): [string, string][] {
	const dir = join(ROOT, language, 'web');
	return readdirSync(dir)
		.filter((file) => file.endsWith('.ftl'))
		.sort()
		.map((file) => [`${language}/web/${file}`, readFileSync(join(dir, file), 'utf8')]);
}

function variablesIn(pattern: Pattern, into: Set<string>) {
	for (const element of pattern.elements) {
		if (element.type === 'Placeable') expressionVariables(element.expression, into);
	}
}

function expressionVariables(expression: Expression | InlineExpression, into: Set<string>) {
	switch (expression.type) {
		case 'VariableReference':
			into.add(expression.id.name);
			break;
		case 'SelectExpression':
			expressionVariables(expression.selector, into);
			for (const variant of expression.variants) variablesIn(variant.value, into);
			break;
		case 'Placeable':
			expressionVariables(expression.expression, into);
			break;
		case 'FunctionReference':
		case 'TermReference':
			for (const argument of expression.arguments?.positional ?? [])
				expressionVariables(argument, into);
			for (const argument of expression.arguments?.named ?? [])
				expressionVariables(argument.value, into);
			break;
		default:
	}
}

function messages(language: string): Map<string, string[]> {
	const found = new Map<string, string[]>();
	for (const [, source] of sources(language)) {
		for (const entry of parse(source, {}).body as Entry[]) {
			if (entry.type !== 'Message') continue;
			const variables = new Set<string>();
			if (entry.value) variablesIn(entry.value, variables);
			for (const attribute of entry.attributes) variablesIn(attribute.value, variables);
			found.set(entry.id.name, [...variables].sort());
		}
	}
	return found;
}

describe('the catalog files', () => {
	it.each(languages)('%s parses without errors', (language) => {
		for (const [name, source] of sources(language)) {
			const junk = parse(source, {}).body.filter((entry) => entry.type === 'Junk');
			expect(
				junk.map((entry) => entry.content),
				name
			).toEqual([]);
		}
		expect(() =>
			createBundle(
				language,
				sources(language).map(([, source]) => source)
			)
		).not.toThrow();
	});

	it('has the default language', () => {
		expect(languages).toContain(DEFAULT_LOCALE);
		expect(LOCALES[0]).toBe(DEFAULT_LOCALE);
	});

	it.each(languages.filter((language) => language !== DEFAULT_LOCALE))(
		'%s has exactly the messages of the default language, with the same placeholders',
		(language) => {
			const reference = messages(DEFAULT_LOCALE);
			const translated = messages(language);
			expect([...translated.keys()].sort()).toEqual([...reference.keys()].sort());
			for (const [id, variables] of reference) {
				expect(translated.get(id), id).toEqual(variables);
			}
		}
	);
});

function walk(dir: string): string[] {
	return readdirSync(dir).flatMap((name) => {
		const path = join(dir, name);
		if (statSync(path).isDirectory()) return name === 'generated' ? [] : walk(path);
		return /\.(svelte|ts)$/.test(name) ? [path] : [];
	});
}

function usage() {
	const literal = new Set<string>();
	const prefixes = new Set<string>();
	for (const file of walk(SRC)) {
		const text = readFileSync(file, 'utf8');
		for (const match of text.matchAll(/\bt\(\s*(['"`])([a-z0-9-]+)(\$\{)?/g)) {
			(match[3] ? prefixes : literal).add(match[2]!);
		}
	}
	return { literal, prefixes, files: walk(SRC).map((file) => relative(SRC, file)) };
}

describe('the catalog and the code', () => {
	const ids = new Set(messages(DEFAULT_LOCALE).keys());
	const { literal, prefixes } = usage();

	it('defines every message the code asks for', () => {
		const missing = [...literal].filter((id) => !ids.has(id));
		expect(missing).toEqual([]);
		for (const prefix of prefixes) {
			expect(
				[...ids].some((id) => id.startsWith(prefix)),
				`${prefix}…`
			).toBe(true);
		}
	});

	it('has no message that nothing uses', () => {
		const unused = [...ids].filter(
			(id) => !literal.has(id) && ![...prefixes].some((prefix) => id.startsWith(prefix))
		);
		expect(unused).toEqual([]);
	});

	it('renders through the runtime the same text the file says', () => {
		expect(i18n.t('common-cancel')).toBe('Cancel');
	});
});
