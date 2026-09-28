import { create, fromBinary } from '@bufbuild/protobuf';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { i18n, t } from '$lib/i18n';
import {
	ABORTED,
	API_BASE,
	ApiClient,
	ApiError,
	INVALID_RESPONSE,
	NETWORK_ERROR,
	PROTOBUF,
	TIMEOUT_ERROR,
	UNKNOWN_ERROR,
	encode,
	message,
	searchParams,
	setErrorListener,
	setReauthenticator,
	setUnauthenticatedHandler
} from '$lib/api';
import {
	CreateNoteRequestSchema,
	NotePageSchema,
	NoteSchema,
	RecoveryCodesSchema,
	VerificationPendingSchema
} from '$lib/types/api';
import {
	callOf,
	mockFetch,
	noContent,
	problem,
	reply,
	sent,
	useEnglish,
	useGerman
} from '../helpers';

afterEach(async () => {
	await useEnglish();
	setUnauthenticatedHandler(null);
	setErrorListener(null);
	setReauthenticator(null);
});

/** A `fetch` that never answers, but rejects like the real one when its signal aborts. */
function hangingFetch() {
	return vi.fn<typeof fetch>(
		(_input, init) =>
			new Promise<Response>((_resolve, reject) => {
				const signal = init?.signal;
				if (signal?.aborted) reject(signal.reason);
				signal?.addEventListener('abort', () => reject(signal.reason));
			})
	);
}

describe('ApiClient', () => {
	it('sends protobuf with the session cookie and the CSRF header', async () => {
		const fetchFn = mockFetch(reply(NoteSchema, { id: '1', title: 'x' }, 201));
		const body = encode(CreateNoteRequestSchema, { title: 'x' });

		const result = await new ApiClient(fetchFn).post('/notes', {
			body,
			response: message(NoteSchema)
		});

		expect(result).toEqual(create(NoteSchema, { id: '1', title: 'x' }));
		const [url, init] = callOf(fetchFn);
		expect(url).toBe(`${API_BASE}/notes`);
		expect(init).toEqual({
			method: 'POST',
			headers: {
				accept: `${PROTOBUF}, application/problem+json`,
				'x-requested-with': 'fetch',
				'accept-language': i18n.locale,
				'content-type': PROTOBUF
			},
			body,
			credentials: 'include',
			signal: expect.any(AbortSignal)
		});
		expect(sent(fetchFn, CreateNoteRequestSchema)).toEqual(
			create(CreateNoteRequestSchema, { title: 'x' })
		);
	});

	it('sends no body or content type without one', async () => {
		const fetchFn = mockFetch(reply(NotePageSchema));

		await new ApiClient(fetchFn).get('/notes');

		const [, init] = callOf(fetchFn);
		expect(init.body).toBeUndefined();
		expect(init.headers).not.toHaveProperty('content-type');
		expect(init.method).toBe('GET');
	});

	it('supports every method', async () => {
		const fetchFn = mockFetch(noContent());
		const client = new ApiClient(fetchFn);
		const body = encode(CreateNoteRequestSchema, { title: 'x' });

		await client.put('/a', { body });
		await client.patch('/a', { body });
		await client.delete('/a');

		expect(fetchFn.mock.calls.map(([, init]) => init?.method)).toEqual(['PUT', 'PATCH', 'DELETE']);
	});

	it('resolves to undefined without a decoder, whatever the body', async () => {
		const client = new ApiClient(mockFetch(noContent()));
		await expect(client.post('/auth/logout')).resolves.toBeUndefined();

		const empty = new ApiClient(
			mockFetch(new Response('', { status: 202, headers: { 'content-length': '0' } }))
		);
		await expect(empty.post('/auth/forgot-password')).resolves.toBeUndefined();

		const html = new ApiClient(mockFetch(new Response('<html>ok</html>', { status: 200 })));
		await expect(html.post('/auth/logout')).resolves.toBeUndefined();
	});

	it('decodes an empty body as the default message', async () => {
		// Protobuf encodes a message whose fields all have their defaults as zero bytes.
		const client = new ApiClient(mockFetch(reply(NotePageSchema), noContent()));
		const page = message(NotePageSchema);

		await expect(client.get('/notes', { response: page })).resolves.toEqual(create(NotePageSchema));
		await expect(client.get('/notes', { response: page })).resolves.toEqual(create(NotePageSchema));
	});

	it('hands the status to the decoder', async () => {
		const decode = vi.fn((status: number, body: Uint8Array) =>
			status === 202 ? fromBinary(VerificationPendingSchema, body).email : 'signed in'
		);
		const client = new ApiClient(
			mockFetch(
				reply(VerificationPendingSchema, { email: 'a@example.com' }, 202),
				reply(NoteSchema)
			)
		);

		await expect(client.post('/auth/register', { response: decode })).resolves.toBe(
			'a@example.com'
		);
		await expect(client.post('/auth/register', { response: decode })).resolves.toBe('signed in');
		expect(decode.mock.calls.map(([status]) => status)).toEqual([202, 200]);
	});

	it('builds the query string from defined values only', async () => {
		const fetchFn = mockFetch(reply(NotePageSchema));

		await new ApiClient(fetchFn).get('/notes', {
			query: { limit: 20, after: undefined, scope: 'all', search: '', flag: null }
		});

		expect(callOf(fetchFn)[0]).toBe(`${API_BASE}/notes?limit=20&scope=all`);
	});

	it('turns problem documents into ApiErrors', async () => {
		const client = new ApiClient(
			mockFetch(
				problem(422, 'validation_failed', {
					detail: 'the request is invalid',
					errors: [
						{ field: 'email', code: 'invalid_email', message: 'enter a valid email address' }
					]
				})
			)
		);

		const error = await client.post('/auth/register').catch((e: unknown) => e);

		expect(error).toBeInstanceOf(ApiError);
		const apiError = error as ApiError;
		expect(apiError.status).toBe(422);
		expect(apiError.code).toBe('validation_failed');
		expect(apiError.message).toBe('the request is invalid');
		expect(apiError.isValidation).toBe(true);
		expect(apiError.fieldError('email')).toBe('enter a valid email address');
		expect(apiError.fieldError('password')).toBeUndefined();
	});

	it('reads Retry-After from rate-limited responses', async () => {
		const response = problem(429, 'rate_limited');
		response.headers.set('retry-after', '120');
		const client = new ApiClient(mockFetch(response));

		const error = (await client.post('/auth/login').catch((e: unknown) => e)) as ApiError;

		expect(error.code).toBe('rate_limited');
		expect(error.retryAfter).toBe(120);
	});

	it('falls back to the status text without a problem document', async () => {
		const client = new ApiClient(
			mockFetch(
				new Response('<html>Bad Gateway</html>', { status: 502, statusText: 'Bad Gateway' })
			)
		);

		const error = (await client.get('/me').catch((e: unknown) => e)) as ApiError;

		expect(error.status).toBe(502);
		expect(error.code).toBe(UNKNOWN_ERROR);
		expect(error.message).toBe('Bad Gateway');
	});

	it('ignores JSON errors that are not problem documents', async () => {
		const client = new ApiClient(mockFetch(Response.json({ error: 'nope' }, { status: 500 })));
		const error = (await client.get('/me').catch((e: unknown) => e)) as ApiError;
		expect(error.code).toBe(UNKNOWN_ERROR);
		expect(error.message).toBe(t('error-request-failed', { status: 500 }));
	});

	it('reports network failures', async () => {
		const client = new ApiClient(
			vi.fn<typeof fetch>(async () => {
				throw new TypeError('Failed to fetch');
			})
		);

		const error = (await client.get('/me').catch((e: unknown) => e)) as ApiError;

		expect(error.status).toBe(0);
		expect(error.code).toBe(NETWORK_ERROR);
		expect(error.message).toBe(t('error-network'));
	});

	it('asks for the language in use, on every request', async () => {
		const fetchFn = mockFetch(noContent());
		const client = new ApiClient(fetchFn);

		await client.get('/me');
		await useGerman();
		await client.get('/me');
		await i18n.use('en');
		await client.get('/me');

		const languages = [0, 1, 2].map((n) =>
			new Headers(callOf(fetchFn, n)[1].headers).get('accept-language')
		);
		expect(languages).toEqual(['en', 'de', 'en']);
	});

	it('runs the unauthenticated handler on 401', async () => {
		const handler = vi.fn();
		setUnauthenticatedHandler(handler);
		const client = new ApiClient(mockFetch(problem(401, 'unauthenticated')));

		await expect(client.get('/notes')).rejects.toBeInstanceOf(ApiError);
		expect(handler).toHaveBeenCalledOnce();
		expect(handler.mock.calls[0]?.[0]).toMatchObject({ code: 'unauthenticated' });

		await expect(client.get('/me', { handleUnauthenticated: false })).rejects.toBeInstanceOf(
			ApiError
		);
		expect(handler).toHaveBeenCalledOnce();
	});

	it('does not run the handler for other errors', async () => {
		const handler = vi.fn();
		setUnauthenticatedHandler(handler);
		const client = new ApiClient(mockFetch(problem(403, 'forbidden')));

		await expect(client.get('/admin/users')).rejects.toMatchObject({ status: 403 });
		expect(handler).not.toHaveBeenCalled();
	});

	it('gives up after the timeout', async () => {
		const error = await new ApiClient(hangingFetch())
			.get('/me', { timeoutMs: 5 })
			.catch((e: unknown) => e);

		expect(error).toMatchObject({ status: 0, code: TIMEOUT_ERROR });
	});

	it('can be cancelled by the caller', async () => {
		const controller = new AbortController();
		const request = new ApiClient(hangingFetch()).get('/notes', { signal: controller.signal });
		controller.abort();

		const error = (await request.catch((e: unknown) => e)) as ApiError;
		expect(error.code).toBe(ABORTED);
		expect(error.isAborted).toBe(true);
	});

	it('combines signals where AbortSignal.any is missing', async () => {
		const any = Object.getOwnPropertyDescriptor(AbortSignal, 'any');
		Object.defineProperty(AbortSignal, 'any', { value: undefined, configurable: true });
		try {
			const aborted = new AbortController();
			aborted.abort();
			await expect(
				new ApiClient(hangingFetch()).get('/notes', { signal: aborted.signal })
			).rejects.toMatchObject({ code: ABORTED });

			const later = new AbortController();
			const request = new ApiClient(hangingFetch()).get('/notes', { signal: later.signal });
			later.abort();
			await expect(request).rejects.toMatchObject({ code: ABORTED });
		} finally {
			if (any) Object.defineProperty(AbortSignal, 'any', any);
		}
	});

	it('reports a successful response that is not protobuf', async () => {
		const client = new ApiClient(
			mockFetch(new Response('<html>proxy page</html>', { status: 200 }))
		);
		const error = await client
			.get('/notes/n1', { response: message(NoteSchema) })
			.catch((e: unknown) => e);
		expect(error).toMatchObject({ status: 200, code: INVALID_RESPONSE });
	});

	it('reports a protobuf response it cannot decode', async () => {
		const client = new ApiClient(
			mockFetch(
				// Field 1 with the varint wire type, then a truncated varint.
				new Response(new Uint8Array([0x08, 0xff]), {
					status: 200,
					headers: { 'content-type': `${PROTOBUF}; charset=binary` }
				})
			)
		);
		const error = await client
			.get('/notes/n1', { response: message(NoteSchema) })
			.catch((e: unknown) => e);
		expect(error).toMatchObject({ status: 200, code: INVALID_RESPONSE });
	});

	it('tells the error listener about failures the 401 handler does not take', async () => {
		const listener = vi.fn();
		const handler = vi.fn();
		setErrorListener(listener);
		setUnauthenticatedHandler(handler);

		await expect(
			new ApiClient(mockFetch(problem(403, 'forbidden'))).post('/notes')
		).rejects.toBeInstanceOf(ApiError);
		expect(listener).toHaveBeenCalledWith(expect.objectContaining({ code: 'forbidden' }));

		await expect(
			new ApiClient(mockFetch(problem(401, 'unauthenticated'))).get('/notes')
		).rejects.toBeInstanceOf(ApiError);
		expect(handler).toHaveBeenCalledOnce();
		expect(listener).toHaveBeenCalledOnce();
	});

	describe('step-up re-authentication', () => {
		const reauth = () => problem(403, 'reauth_required');

		it('asks the user, then sends the request once more', async () => {
			const reauthenticator = vi.fn(async () => true);
			setReauthenticator(reauthenticator);
			const fetchFn = mockFetch(reauth(), reply(RecoveryCodesSchema, { codes: ['a'] }));

			await expect(
				new ApiClient(fetchFn).post('/me/mfa/recovery-codes', {
					response: message(RecoveryCodesSchema)
				})
			).resolves.toEqual(create(RecoveryCodesSchema, { codes: ['a'] }));
			expect(reauthenticator).toHaveBeenCalledOnce();
			expect(fetchFn).toHaveBeenCalledTimes(2);
		});

		it('retries only once', async () => {
			setReauthenticator(async () => true);
			const fetchFn = mockFetch(reauth());

			await expect(new ApiClient(fetchFn).delete('/me/passkeys/p1')).rejects.toMatchObject({
				code: 'reauth_required'
			});
			expect(fetchFn).toHaveBeenCalledTimes(2);
		});

		it('gives the error to the caller when the user cancels, or nobody can ask', async () => {
			const fetchFn = mockFetch(reauth());
			await expect(new ApiClient(fetchFn).post('/me/email')).rejects.toMatchObject({
				code: 'reauth_required'
			});

			setReauthenticator(async () => false);
			await expect(new ApiClient(fetchFn).post('/me/email')).rejects.toMatchObject({
				code: 'reauth_required'
			});
			expect(fetchFn).toHaveBeenCalledTimes(2);
		});

		it('can be turned off per request', async () => {
			const reauthenticator = vi.fn(async () => true);
			setReauthenticator(reauthenticator);
			await expect(
				new ApiClient(mockFetch(reauth())).post('/me/email', { reauthenticate: false })
			).rejects.toMatchObject({ code: 'reauth_required' });
			expect(reauthenticator).not.toHaveBeenCalled();
		});

		it('asks once for requests that fail together', async () => {
			let confirm: (done: boolean) => void = () => {};
			const reauthenticator = vi.fn(() => new Promise<boolean>((r) => (confirm = r)));
			setReauthenticator(reauthenticator);
			const client = new ApiClient(mockFetch(reauth(), reauth(), noContent()));

			const both = Promise.all([client.post('/me/phone'), client.post('/me/phone')]);
			await vi.waitFor(() => expect(reauthenticator).toHaveBeenCalledOnce());
			confirm(true);
			await expect(both).resolves.toEqual([undefined, undefined]);
			expect(reauthenticator).toHaveBeenCalledOnce();
		});
	});

	it('uses the global fetch by default', async () => {
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		try {
			await new ApiClient().get('/health');
			expect(fetchFn).toHaveBeenCalledOnce();
		} finally {
			vi.unstubAllGlobals();
		}
	});
});

describe('searchParams', () => {
	it('is empty without values', () => {
		expect(searchParams(undefined)).toBe('');
		expect(searchParams({ a: undefined })).toBe('');
	});

	it('encodes values', () => {
		expect(searchParams({ search: 'a b&c', flag: true })).toBe('?search=a+b%26c&flag=true');
	});
});
