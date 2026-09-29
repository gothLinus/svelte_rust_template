import {
	CreateNoteRequestSchema,
	NotePageSchema,
	NoteSchema,
	UpdateNoteRequestSchema
} from '$lib/types/api';
import { type ApiClient, type Init, encode, message, segment } from './client';

export type NoteScope = 'mine' | 'all';

export interface ListNotesQuery {
	scope?: NoteScope;
	limit?: number;
	after?: string;
}

export function notesApi(client: ApiClient) {
	return {
		list: (query: ListNotesQuery = {}) =>
			client.get('/notes', { query: { ...query }, response: message(NotePageSchema) }),
		get: (id: string) => client.get(`/notes/${segment(id)}`, { response: message(NoteSchema) }),
		create: (body: Init<typeof CreateNoteRequestSchema>) =>
			client.post('/notes', {
				body: encode(CreateNoteRequestSchema, body),
				response: message(NoteSchema)
			}),
		update: (id: string, body: Init<typeof UpdateNoteRequestSchema>) =>
			client.patch(`/notes/${segment(id)}`, {
				body: encode(UpdateNoteRequestSchema, body),
				response: message(NoteSchema)
			}),
		remove: (id: string) => client.delete(`/notes/${segment(id)}`),
		duplicate: (id: string) =>
			client.post(`/notes/${segment(id)}/duplicate`, { response: message(NoteSchema) })
	};
}
