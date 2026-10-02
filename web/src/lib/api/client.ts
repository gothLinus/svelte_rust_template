import {
	create,
	fromBinary,
	toBinary,
	type DescMessage,
	type MessageInitShape,
	type MessageShape
} from '@bufbuild/protobuf';
import { i18n, t } from '$lib/i18n';
import { ApiError, REAUTH_REQUIRED, UNKNOWN_ERROR, isProblemDetails } from './errors';

export const API_BASE = '/api/v1';

export const DEFAULT_TIMEOUT_MS = 15_000;

export const PROTOBUF = 'application/x-protobuf';

export type Method = 'GET' | 'POST' | 'PUT' | 'PATCH' | 'DELETE';
export type Query = Record<string, string | number | boolean | null | undefined>;

/**
 * Reads a successful response body. Most endpoints answer with one message type
 * (`message(MeSchema)`); a few pick it by status, e.g. `202` for a second sign-in step.
 */
export type Decoder<T> = (status: number, body: Uint8Array) => T;

export function message<Desc extends DescMessage>(schema: Desc): Decoder<MessageShape<Desc>> {
	return (_status, body) => fromBinary(schema, body);
}

/**
 * The body as a `Blob` of `type`, for downloads that are not protobuf (a JSON export).
 * Pass the same `type` as the request's `accept`.
 */
export function blob(type: string): Decoder<Blob> {
	return (_status, body) => new Blob([body.slice()], { type });
}

export type Init<Desc extends DescMessage> = MessageInitShape<Desc>;

export function encode<Desc extends DescMessage>(
	schema: Desc,
	init: MessageInitShape<Desc>
): Uint8Array<ArrayBuffer> {
	return toBinary(schema, create(schema, init));
}

export interface RequestOptions<T = void> {
	query?: Query;
	body?: Uint8Array<ArrayBuffer>;
	/** Extra request headers, such as `if-match` (see `ifMatch`). */
	headers?: Record<string, string>;
	response?: Decoder<T>;
	/**
	 * The content type a successful response must have (default `PROTOBUF`), for the few
	 * endpoints that answer with a file, read with `blob`.
	 */
	accept?: string;
	/**
	 * Run the unauthenticated handler on a 401 (default `true`). Turn it off where a 401 is
	 * an expected answer, such as asking "who am I?" on startup.
	 */
	handleUnauthenticated?: boolean;
	signal?: AbortSignal;
	timeoutMs?: number;
	/**
	 * On `403 reauth_required`, ask the user to confirm it is them and send the request
	 * once more (default `true`).
	 */
	reauthenticate?: boolean;
}

export type UnauthenticatedHandler = (error: ApiError) => void;
export type ErrorListener = (error: ApiError) => void;
export type Reauthenticator = () => Promise<boolean>;

let onUnauthenticated: UnauthenticatedHandler | null = null;
let onError: ErrorListener | null = null;
let reauthenticator: Reauthenticator | null = null;
let reauthenticating: Promise<boolean> | null = null;

/**
 * Sets what happens when a request comes back 401 because the session ended (expired, or
 * revoked elsewhere). The root layout installs a handler that clears the session and
 * sends the user to the login page.
 */
export function setUnauthenticatedHandler(handler: UnauthenticatedHandler | null): void {
	onUnauthenticated = handler;
}

/**
 * Sets a listener for every failed response except the 401s the unauthenticated handler
 * takes. The one place for reactions that do not belong to a single form, keyed on
 * `error.code` (see `handleApiError` in `$lib/auth`). The caller still gets the error.
 */
export function setErrorListener(listener: ErrorListener | null): void {
	onError = listener;
}

/**
 * Sets how the user proves again who they are when a sensitive request answers
 * `403 reauth_required` (step-up re-authentication). The request is then retried once, so
 * callers never see that error unless the user cancels. The root layout installs a dialog.
 */
export function setReauthenticator(handler: Reauthenticator | null): void {
	reauthenticator = handler;
	reauthenticating = null;
}

function reauthenticate(): Promise<boolean> {
	if (!reauthenticator) return Promise.resolve(false);
	reauthenticating ??= reauthenticator().finally(() => (reauthenticating = null));
	return reauthenticating;
}

/**
 * The one wrapper around `fetch` that talks to the API.
 *
 * - Sends and receives Protocol Buffers messages (`application/x-protobuf`), with the session
 *   cookie (`credentials: 'include'`).
 * - Sends `X-Requested-With`, which the server requires on state-changing requests as CSRF
 *   protection: cross-site forms cannot set custom headers.
 * - Turns every failure into an `ApiError`, parsed from the RFC 9457 problem document (JSON,
 *   unlike the successful responses).
 *
 * Inside a `load` function pass SvelteKit's `fetch`, so requests are tracked for
 * invalidation: `new ApiClient(fetch)`.
 */
export class ApiClient {
	readonly #fetch: typeof fetch;

	constructor(fetchFn: typeof fetch = (input, init) => fetch(input, init)) {
		this.#fetch = fetchFn;
	}

	get<T = void>(path: string, options: Omit<RequestOptions<T>, 'body'> = {}): Promise<T> {
		return this.request('GET', path, options);
	}

	post<T = void>(path: string, options: RequestOptions<T> = {}): Promise<T> {
		return this.request('POST', path, options);
	}

	put<T = void>(path: string, options: RequestOptions<T> = {}): Promise<T> {
		return this.request('PUT', path, options);
	}

	patch<T = void>(path: string, options: RequestOptions<T> = {}): Promise<T> {
		return this.request('PATCH', path, options);
	}

	delete<T = void>(path: string, options: RequestOptions<T> = {}): Promise<T> {
		return this.request('DELETE', path, options);
	}

	async request<T = void>(
		method: Method,
		path: string,
		options: RequestOptions<T> = {}
	): Promise<T> {
		try {
			return await this.#send<T>(method, path, options);
		} catch (error) {
			const retry =
				error instanceof ApiError &&
				error.code === REAUTH_REQUIRED &&
				options.reauthenticate !== false &&
				(await reauthenticate());
			if (!retry) throw error;
			return this.#send<T>(method, path, options);
		}
	}

	async #send<T>(method: Method, path: string, options: RequestOptions<T>): Promise<T> {
		const {
			query,
			body,
			response: decode,
			handleUnauthenticated = true,
			signal,
			timeoutMs,
			accept = PROTOBUF,
			headers: extraHeaders
		} = options;
		const headers: Record<string, string> = {
			...extraHeaders,
			accept: `${accept}, application/problem+json`,
			'x-requested-with': 'fetch',
			'accept-language': i18n.locale
		};
		if (body !== undefined) headers['content-type'] = PROTOBUF;
		const timeout = AbortSignal.timeout(timeoutMs ?? DEFAULT_TIMEOUT_MS);

		let response: Response;
		let bytes: Uint8Array;
		try {
			response = await this.#fetch(`${API_BASE}${path}${searchParams(query)}`, {
				method,
				headers,
				body,
				credentials: 'include',
				signal: signal ? anySignal([signal, timeout]) : timeout
			});
			if (!response.ok) {
				const error = await toApiError(response);
				if (error.status === 401 && handleUnauthenticated) onUnauthenticated?.(error);
				else onError?.(error);
				throw error;
			}
			// The timeout covers reading the body too, so a stalled stream cannot hang a form.
			bytes =
				response.status === 204 ? new Uint8Array() : new Uint8Array(await response.arrayBuffer());
		} catch (error) {
			if (error instanceof ApiError) throw error;
			if (timeout.aborted) throw ApiError.timeout();
			if (signal?.aborted) throw ApiError.aborted();
			throw ApiError.network();
		}

		if (!decode) return undefined as T;
		if (bytes.length > 0 && !hasType(response, accept)) {
			throw ApiError.invalidResponse(response.status);
		}
		try {
			return decode(response.status, bytes);
		} catch {
			throw ApiError.invalidResponse(response.status);
		}
	}
}

function hasType(response: Response, expected: string): boolean {
	const type = response.headers.get('content-type')?.split(';')[0]?.trim().toLowerCase();
	return type === expected;
}

function anySignal(signals: AbortSignal[]): AbortSignal {
	if (typeof AbortSignal.any === 'function') return AbortSignal.any(signals);
	const controller = new AbortController();
	for (const signal of signals) {
		if (signal.aborted) controller.abort(signal.reason);
		else signal.addEventListener('abort', () => controller.abort(signal.reason), { once: true });
	}
	return controller.signal;
}

async function toApiError(response: Response): Promise<ApiError> {
	const retryAfter = Number(response.headers.get('retry-after')) || null;
	try {
		const body: unknown = await response.json();
		if (isProblemDetails(body)) return ApiError.fromProblem(body, retryAfter);
	} catch {
		// Not JSON, e.g. the dev proxy failing to reach the server.
	}
	return new ApiError(
		response.status,
		UNKNOWN_ERROR,
		response.statusText || t('error-request-failed', { status: response.status }),
		[],
		retryAfter
	);
}

export function searchParams(query: Query | undefined): string {
	if (!query) return '';
	const params = new URLSearchParams();
	for (const [key, value] of Object.entries(query)) {
		if (value !== undefined && value !== null && value !== '') params.set(key, String(value));
	}
	const search = params.toString();
	return search ? `?${search}` : '';
}

/**
 * Makes an update or delete apply only to the `version` the client read: the server answers
 * `412` with code `stale` (`STALE`) if the item changed since.
 */
export function ifMatch(version: string): Record<string, string> {
	return { 'if-match': `"${version}"` };
}

export function segment(id: string): string {
	return encodeURIComponent(id);
}
