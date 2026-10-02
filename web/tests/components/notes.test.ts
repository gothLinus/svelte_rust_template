import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { toast } from 'svelte-sonner';
import { session } from '$lib/auth';
import { i18n, t } from '$lib/i18n';
import NoteDialog from '$lib/components/note-dialog.svelte';
import {
	type Note,
	NotePageSchema,
	NoteSchema,
	Permission,
	UpdateNoteRequestSchema
} from '$lib/types/api';
import NotesPage from '../../src/routes/(app)/notes/+page.svelte';
import { create } from '@bufbuild/protobuf';
import {
	OTHER_ID,
	callOf,
	me,
	mockFetch,
	noContent,
	note,
	problem,
	reply,
	rootData,
	sent
} from '../helpers';
import { navigation, visit } from './fake-app.svelte';

describe('note dialog', () => {
	it('starts from the note on every opening', async () => {
		const user = userEvent.setup();
		const onsaved = vi.fn();
		const view = render(NoteDialog, { open: true, note: note(), onsaved });

		const title = screen.getByLabelText(t('note-field-title'));
		expect(title).toHaveValue('Groceries');
		await user.clear(title);
		await user.type(title, 'Changed');
		await user.click(screen.getByRole('button', { name: t('common-cancel') }));
		await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());

		await view.rerender({ open: true, note: null, onsaved });
		expect(await screen.findByLabelText(t('note-field-title'))).toHaveValue('');
		expect(screen.getByRole('heading', { name: t('note-title-new') })).toBeInTheDocument();
	});

	it('validates, then saves the changes', async () => {
		const user = userEvent.setup();
		const saved = note(undefined, { title: 'Shopping' });
		const fetchFn = mockFetch(reply(NoteSchema, saved));
		vi.stubGlobal('fetch', fetchFn);
		const onsaved = vi.fn();
		render(NoteDialog, { open: true, note: note(), onsaved });

		const title = screen.getByLabelText(t('note-field-title'));
		await user.clear(title);
		await user.click(screen.getByRole('button', { name: t('note-save') }));
		expect(title).toHaveAccessibleDescription(t('validation-required'));
		expect(title).toHaveFocus();
		expect(fetchFn).not.toHaveBeenCalled();

		await user.type(title, 'Shopping');
		await user.click(screen.getByRole('button', { name: t('note-save') }));

		await vi.waitFor(() => expect(onsaved).toHaveBeenCalledWith(saved, false));
		const [url, init] = callOf(fetchFn);
		expect(url).toBe(`/api/v1/notes/${note().id}`);
		expect(init.method).toBe('PATCH');
		expect(init.headers).toMatchObject({ 'if-match': '"1"' });
		expect(sent(fetchFn, UpdateNoteRequestSchema)).toEqual(
			create(UpdateNoteRequestSchema, { title: 'Shopping', body: 'milk' })
		);
	});

	it('shows the server field errors', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(
				problem(422, 'validation_failed', {
					errors: [{ field: 'body', code: 'too_long', message: 'must be shorter' }]
				})
			)
		);
		render(NoteDialog, { open: true, note: null, onsaved: vi.fn() });

		await user.type(screen.getByLabelText(t('note-field-title')), 'Groceries');
		await user.click(screen.getByRole('button', { name: t('note-create') }));

		const body = screen.getByLabelText(t('note-field-text'));
		await vi.waitFor(() => expect(body).toHaveAccessibleDescription('must be shorter'));
		expect(body).toHaveFocus();
	});
});

describe('note dialog when someone else saved first', () => {
	it('keeps the edits and saves them over the newer version on purpose', async () => {
		const user = userEvent.setup();
		const newer = note(undefined, { title: 'Theirs', version: '2' });
		const saved = note(undefined, { title: 'Mine', version: '3' });
		const fetchFn = mockFetch(
			problem(412, 'stale'),
			reply(NoteSchema, newer),
			reply(NoteSchema, saved)
		);
		vi.stubGlobal('fetch', fetchFn);
		const onsaved = vi.fn();
		render(NoteDialog, { open: true, note: note(), onsaved });

		const title = screen.getByLabelText(t('note-field-title'));
		await user.clear(title);
		await user.type(title, 'Mine');
		await user.click(screen.getByRole('button', { name: t('note-save') }));

		expect(await screen.findByText(t('note-stale'))).toBeInTheDocument();
		expect(onsaved).not.toHaveBeenCalled();
		expect(title).toHaveValue('Mine');
		expect(callOf(fetchFn, 1)[1].method ?? 'GET').toBe('GET');

		await user.click(screen.getByRole('button', { name: t('note-save') }));
		await vi.waitFor(() => expect(onsaved).toHaveBeenCalledWith(saved, false));
		expect(callOf(fetchFn, 2)[1].headers).toMatchObject({ 'if-match': '"2"' });
	});
});

describe('notes page', () => {
	const page = (items: Note[], after: string | null, nextCursor?: string) => ({
		data: {
			...rootData(me([Permission.NOTES_READ, Permission.NOTES_WRITE])),
			notes: create(NotePageSchema, { items, nextCursor }),
			scope: 'mine' as const,
			after
		}
	});

	beforeEach(() => {
		visit('/notes');
		session.connect(() => me([Permission.NOTES_READ, Permission.NOTES_WRITE]));
	});

	it('keeps Previous right through Next, Back and Forward', async () => {
		const user = userEvent.setup();
		const view = render(NotesPage, page([note()], null, 'c1'));

		const previous = () => screen.getByRole('button', { name: t('pagination-previous') });
		expect(previous()).toBeDisabled();

		await user.click(screen.getByRole('button', { name: t('pagination-next') }));
		expect(navigation.goto).toHaveBeenLastCalledWith('/notes?after=c1', expect.anything());
		visit('/notes?after=c1');
		await view.rerender(page([note()], 'c1', 'c2'));

		await user.click(screen.getByRole('button', { name: t('pagination-next') }));
		visit('/notes?after=c2');
		await view.rerender(page([note()], 'c2'));

		visit('/notes');
		await view.rerender(page([note()], null, 'c1'));
		expect(previous()).toBeDisabled();

		visit('/notes?after=c1');
		await view.rerender(page([note()], 'c1', 'c2'));
		await user.click(previous());
		expect(navigation.goto).toHaveBeenLastCalledWith('/notes', expect.anything());
	});

	it('shows only the actions the user may take, with unique names', () => {
		render(NotesPage, page([note(), note(OTHER_ID, { id: 'n2', title: 'Theirs' })], null));

		const [mine, theirs] = screen.getAllByRole('listitem');
		expect(within(mine!).getByRole('heading', { level: 2, name: 'Groceries' })).toBeTruthy();
		expect(
			within(mine!).getByRole('button', { name: t('notes-edit-label', { title: 'Groceries' }) })
		).toBeInTheDocument();
		expect(
			within(theirs!).queryByRole('button', { name: new RegExp(t('notes-delete')) })
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole('navigation', { name: t('notes-pages-label') })
		).not.toBeInTheDocument();
	});

	it('asks before deleting', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		const success = vi.spyOn(toast, 'success');
		render(NotesPage, page([note()], null));

		await user.click(
			screen.getByRole('button', { name: t('notes-delete-label', { title: 'Groceries' }) })
		);
		const dialog = await screen.findByRole('alertdialog');
		expect(dialog).toHaveTextContent(t('notes-delete-description', { title: 'Groceries' }));
		expect(fetchFn).not.toHaveBeenCalled();

		await user.click(within(dialog).getByRole('button', { name: t('notes-delete-confirm') }));
		await vi.waitFor(() => expect(callOf(fetchFn)[1].method).toBe('DELETE'));
		expect(success).toHaveBeenCalledWith(t('notes-deleted', { title: 'Groceries' }));
		expect(navigation.invalidate).toHaveBeenCalledWith('app:notes');
	});

	it('creates a note and switches whose notes to show', async () => {
		const user = userEvent.setup();
		const manager = me([Permission.NOTES_READ, Permission.NOTES_WRITE, Permission.NOTES_MANAGE]);
		session.connect(() => manager);
		vi.stubGlobal('fetch', mockFetch(reply(NoteSchema, note(undefined, { title: 'New' }), 201)));
		visit('/notes?after=c1');
		render(NotesPage, { data: { ...page([], 'c1').data, me: manager } });

		expect(screen.getByText(t('notes-empty-title'))).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: t('notes-scope-all') }));
		expect(navigation.goto).toHaveBeenLastCalledWith('/notes?scope=all', expect.anything());

		await user.click(screen.getByRole('button', { name: t('notes-new') }));
		await user.type(await screen.findByLabelText(t('note-field-title')), 'New');
		await user.click(screen.getByRole('button', { name: t('note-create') }));
		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenLastCalledWith('/notes', expect.anything())
		);
	});

	it('duplicates a note, and offers it only to those who may create', async () => {
		const user = userEvent.setup();
		const copy = note(undefined, { id: 'n2', title: 'Groceries (copy)' });
		const fetchFn = mockFetch(reply(NoteSchema, copy, 201), problem(404, 'not_found'));
		vi.stubGlobal('fetch', fetchFn);
		const success = vi.spyOn(toast, 'success');
		const view = render(NotesPage, page([note()], null));

		await user.click(
			screen.getByRole('button', { name: t('notes-duplicate-label', { title: 'Groceries' }) })
		);
		await vi.waitFor(() => expect(navigation.invalidate).toHaveBeenCalledWith('app:notes'));
		expect(success).toHaveBeenCalledWith(t('notes-created', { title: 'Groceries (copy)' }));
		const [url, init] = callOf(fetchFn);
		expect(url).toBe(`/api/v1/notes/${note().id}/duplicate`);
		expect(init.method).toBe('POST');

		await user.click(
			screen.getByRole('button', { name: t('notes-duplicate-label', { title: 'Groceries' }) })
		);
		await vi.waitFor(() => expect(fetchFn).toHaveBeenCalledTimes(2));
		expect(navigation.invalidate).toHaveBeenCalledOnce();

		view.unmount();
		session.connect(() => me([Permission.NOTES_READ]));
		render(NotesPage, {
			data: { ...page([note()], null).data, me: me([Permission.NOTES_READ]) }
		});
		expect(
			screen.queryByRole('button', { name: new RegExp(t('notes-duplicate')) })
		).not.toBeInTheDocument();
	});

	it('edits a note and reports a failed delete', async () => {
		const user = userEvent.setup();
		vi.stubGlobal('fetch', mockFetch(reply(NoteSchema, note()), problem(404, 'not_found')));
		const success = vi.spyOn(toast, 'success');
		render(NotesPage, page([note()], null));

		await user.click(
			screen.getByRole('button', { name: t('notes-edit-label', { title: 'Groceries' }) })
		);
		await user.click(await screen.findByRole('button', { name: t('note-save') }));
		await vi.waitFor(() => expect(navigation.invalidate).toHaveBeenCalledWith('app:notes'));
		expect(success).toHaveBeenCalledWith(t('notes-saved'));

		document.body.removeAttribute('style');
		await user.click(
			screen.getByRole('button', { name: t('notes-delete-label', { title: 'Groceries' }) })
		);
		await user.click(
			within(await screen.findByRole('alertdialog')).getByRole('button', {
				name: t('notes-delete-confirm')
			})
		);
		await vi.waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
		expect(navigation.invalidate).toHaveBeenCalledOnce();
	});
});

describe('notes in another language', () => {
	afterEach(() => i18n.use('en'));

	it('renders the page again when the language changes', async () => {
		session.connect(() => me([Permission.NOTES_READ, Permission.NOTES_WRITE]));
		// One message per source: `just new-resource` renames the ids in each.
		i18n.register('tlh', [
			'notes-title = Notizen',
			'notes-new = Neue Notiz',
			'notes-edit-label = { $title } bearbeiten'
		]);
		visit('/notes');
		const data = {
			...rootData(me([Permission.NOTES_READ, Permission.NOTES_WRITE])),
			notes: create(NotePageSchema, { items: [note()] }),
			scope: 'mine' as const,
			after: null
		};
		render(NotesPage, { data });
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(t('notes-title'));

		await i18n.use('tlh');
		await tick();

		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent('Notizen');
		expect(screen.getByRole('button', { name: 'Neue Notiz' })).toBeInTheDocument();
		expect(screen.getByRole('button', { name: 'Groceries bearbeiten' })).toBeInTheDocument();
		expect(
			screen.getByRole('button', { name: t('notes-delete-label', { title: 'Groceries' }) })
		).toBeInTheDocument();
	});
});
