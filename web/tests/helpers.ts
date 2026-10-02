import {
	create,
	fromBinary,
	toBinary,
	type DescMessage,
	type MessageInitShape,
	type MessageShape
} from '@bufbuild/protobuf';
import { type Timestamp, timestampFromDate } from '@bufbuild/protobuf/wkt';
import { vi } from 'vitest';
import { i18n } from '$lib/i18n';
import { PROTOBUF, type ProblemDetails, type SignedIn } from '$lib/api';
import {
	type AuthMethods,
	AuthMethodsSchema,
	MeSchema,
	type Note,
	NoteSchema,
	Permission,
	type User,
	UserSchema
} from '$lib/types/api';

export const USER_ID = '01900000-0000-7000-8000-000000000001';
export const OTHER_ID = '01900000-0000-7000-8000-000000000002';

export function ts(iso: string): Timestamp {
	return timestampFromDate(new Date(iso));
}

export function makeUser(overrides: MessageInitShape<typeof UserSchema> = {}): User {
	return create(UserSchema, {
		id: USER_ID,
		email: 'alice@example.com',
		username: 'alice',
		phoneVerified: false,
		emailVerified: true,
		hasPassword: true,
		disabled: false,
		roles: ['user'],
		createdAt: ts('2026-01-01T00:00:00Z'),
		...overrides
	});
}

export function me(
	permissions: Permission[] = [Permission.NOTES_READ, Permission.NOTES_WRITE],
	id = USER_ID,
	overrides: MessageInitShape<typeof UserSchema> = {}
): SignedIn {
	return Object.assign(create(MeSchema, { permissions }), { user: makeUser({ id, ...overrides }) });
}

export function methods(overrides: MessageInitShape<typeof AuthMethodsSchema> = {}): AuthMethods {
	return create(AuthMethodsSchema, { appName: 'Acme', ...overrides });
}

export function rootData<U extends SignedIn | null = null>(
	signedIn: U = null as U,
	overrides: MessageInitShape<typeof AuthMethodsSchema> = {}
) {
	return { me: signedIn, sessionError: null, methods: methods(overrides) };
}

export function note(ownerId = USER_ID, overrides: MessageInitShape<typeof NoteSchema> = {}): Note {
	return create(NoteSchema, {
		id: '01900000-0000-7000-8000-00000000000a',
		ownerId,
		title: 'Groceries',
		body: 'milk',
		createdAt: ts('2026-01-01T00:00:00Z'),
		updatedAt: ts('2026-01-01T00:00:00Z'),
		version: '1',
		...overrides
	});
}

export function reply<Desc extends DescMessage>(
	schema: Desc,
	init: MessageInitShape<Desc> = {} as MessageInitShape<Desc>,
	status = 200
): Response {
	return new Response(toBinary(schema, create(schema, init)), {
		status,
		headers: { 'content-type': PROTOBUF }
	});
}

export function noContent(): Response {
	return new Response(null, { status: 204 });
}

export function sent<Desc extends DescMessage>(
	fetchFn: ReturnType<typeof mockFetch>,
	schema: Desc,
	n = 0
): MessageShape<Desc> {
	const { body } = callOf(fetchFn, n)[1];
	if (!(body instanceof Uint8Array)) throw new Error(`call ${n + 1} sent no protobuf body`);
	return fromBinary(schema, body);
}

export function problem(
	status: number,
	code: string,
	extra: Partial<ProblemDetails> = {}
): Response {
	const body: ProblemDetails = {
		type: 'about:blank',
		title: 'Error',
		status,
		code,
		detail: `problem ${code}`,
		...extra
	};
	return new Response(JSON.stringify(body), {
		status,
		headers: { 'content-type': 'application/problem+json' }
	});
}

export function mockFetch(...responses: Response[]) {
	let call = 0;
	return vi.fn<typeof fetch>(async () => {
		const response = responses[Math.min(call, responses.length - 1)];
		call += 1;
		if (!response) throw new Error('mockFetch needs at least one response');
		return response.clone();
	});
}

export function routeFetch(routes: Record<string, Response>) {
	return vi.fn<typeof fetch>(async (input) => {
		const path = new URL(String(input), 'http://app.test').pathname.replace(/^\/api\/v1/, '');
		return routes[path]?.clone() ?? problem(404, 'not_found');
	});
}

export function callOf(fetchFn: ReturnType<typeof mockFetch>, n = 0): [string, RequestInit] {
	const call = fetchFn.mock.calls[n];
	if (!call) throw new Error(`fetch was not called ${n + 1} times`);
	return [String(call[0]), call[1] ?? {}];
}

/** Builds a partial `load` event. `load` functions only read what each test provides. */
export function loadEvent<E>(event: Record<string, unknown>): E {
	return event as unknown as E;
}

/**
 * A second language for tests, to check that words and formats follow the reader's
 * language: German, with a handful of messages. Everything else falls back to English, as
 * it does for a language that is not fully translated. Switch back with `useEnglish()`.
 */
export const GERMAN = `
session-device = { $browser } auf { $system }
session-device-unknown = Unbekanntes Gerät
nav-dashboard = Übersicht
nav-workspace = Arbeitsbereich
site-description = Ein Starter mit Konten.
error-network = Der Server ist nicht erreichbar.
security-sessions-title = Aktive Sitzungen
security-sessions-sign-out = Abmelden
security-sessions-active = Aktiv { $when }
`;

export async function useGerman(): Promise<void> {
	i18n.register('de', [GERMAN]);
	await i18n.use('de');
}

export async function useEnglish(): Promise<void> {
	await i18n.use('en');
}
