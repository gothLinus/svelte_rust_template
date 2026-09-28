import { adminApi } from './admin';
import { authApi } from './auth';
import { ApiClient } from './client';
import { meApi } from './me';
import { notesApi } from './notes';

export {
	ApiClient,
	API_BASE,
	DEFAULT_TIMEOUT_MS,
	PROTOBUF,
	blob,
	encode,
	message,
	searchParams,
	setErrorListener,
	setReauthenticator,
	setUnauthenticatedHandler
} from './client';
export type {
	Decoder,
	ErrorListener,
	Init,
	Query,
	Reauthenticator,
	RequestOptions,
	UnauthenticatedHandler
} from './client';
export {
	ABORTED,
	ApiError,
	INVALID_RESPONSE,
	NETWORK_ERROR,
	REAUTH_REQUIRED,
	TIMEOUT_ERROR,
	UNKNOWN_ERROR,
	errorMessage,
	isProblemDetails
} from './errors';
export type { FieldError, ProblemDetails } from './errors';
export type { ListUsersQuery } from './admin';
export type { SignedIn } from './me';
export type { ListNotesQuery, NoteScope } from './notes';
export { oauthLinkUrl, oauthUrl } from './auth';
export type { LoginResult, RegisterResult } from './auth';

/**
 * The typed API, one module per resource. Inside a `load` function pass SvelteKit's
 * `fetch`: `createApi(fetch)`. Elsewhere use the shared `api`.
 */
export function createApi(fetchFn?: typeof fetch) {
	const client = new ApiClient(fetchFn);
	return {
		auth: authApi(client),
		me: meApi(client),
		notes: notesApi(client),
		admin: adminApi(client)
	};
}

export type Api = ReturnType<typeof createApi>;

export const api: Api = createApi();
