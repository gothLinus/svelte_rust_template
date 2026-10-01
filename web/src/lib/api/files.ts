import {
	FileUsageSchema,
	StoredFilePageSchema,
	StoredFileSchema,
	UpdateFileRequestSchema
} from '$lib/types/api';
import { API_BASE, type ApiClient, encode, message, segment } from './client';

export type FileScope = 'mine' | 'all';

export interface ListFilesQuery {
	scope?: FileScope;
	limit?: number;
	after?: string;
}

/**
 * How long an upload may take before it fails with `timeout`: the server's default
 * `UPLOAD_TIMEOUT`, since the request's body is the whole file.
 */
export const UPLOAD_TIMEOUT_MS = 10 * 60 * 1000;

/**
 * Files: descriptions travel as messages, contents as raw bytes. An upload's body is the
 * file itself, which `fetch` streams with its length; a download is a plain link to
 * `contentUrl`, so the browser saves it to disk without the app holding it in memory.
 */
export function filesApi(client: ApiClient) {
	return {
		list: (query: ListFilesQuery = {}) =>
			client.get('/files', { query: { ...query }, response: message(StoredFilePageSchema) }),
		get: (id: string) =>
			client.get(`/files/${segment(id)}`, { response: message(StoredFileSchema) }),
		usage: () => client.get('/files/usage', { response: message(FileUsageSchema) }),
		upload: (file: Blob, name: string) =>
			client.post('/files', {
				query: { name },
				body: file,
				contentType: file.type || 'application/octet-stream',
				response: message(StoredFileSchema),
				timeoutMs: UPLOAD_TIMEOUT_MS
			}),
		rename: (id: string, name: string) =>
			client.patch(`/files/${segment(id)}`, {
				body: encode(UpdateFileRequestSchema, { name }),
				response: message(StoredFileSchema)
			}),
		remove: (id: string) => client.delete(`/files/${segment(id)}`)
	};
}

export function contentUrl(id: string): string {
	return `${API_BASE}/files/${segment(id)}/content`;
}
