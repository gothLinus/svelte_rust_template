import { create } from '@bufbuild/protobuf';
import { render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { toast } from 'svelte-sonner';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { contentUrl } from '$lib/api';
import { session } from '$lib/auth';
import FileRenameDialog from '$lib/components/file-rename-dialog.svelte';
import { MAX_FILE_SIZE } from '$lib/forms/validation';
import { formatBytes } from '$lib/helpers/format';
import { t } from '$lib/i18n';
import {
	FileUsageSchema,
	Permission,
	type StoredFile,
	StoredFilePageSchema,
	StoredFileSchema,
	UpdateFileRequestSchema
} from '$lib/types/api';
import FilesPage from '../../src/routes/(app)/files/+page.svelte';
import {
	FILE_USER,
	OTHER_ID,
	callOf,
	me,
	mockFetch,
	noContent,
	problem,
	reply,
	rootData,
	sent,
	storedFile
} from '../helpers';
import { navigation, visit } from './fake-app.svelte';

const MANAGER = [...FILE_USER, Permission.FILES_MANAGE];

function page(
	items: StoredFile[],
	{
		after = null,
		nextCursor,
		quota,
		permissions = FILE_USER
	}: {
		after?: string | null;
		nextCursor?: string;
		quota?: bigint;
		permissions?: Permission[];
	} = {}
) {
	return {
		data: {
			...rootData(me(permissions)),
			files: create(StoredFilePageSchema, { items, nextCursor }),
			usage: create(FileUsageSchema, { usedBytes: 1536n, quotaBytes: quota }),
			scope: 'mine' as const,
			after
		}
	};
}

function picked(name: string, size?: number): File {
	const file = new File(['%PDF'], name, { type: 'application/pdf' });
	if (size !== undefined) Object.defineProperty(file, 'size', { value: size });
	return file;
}

function picker(): HTMLInputElement {
	return screen.getByLabelText(t('files-upload-label'));
}

describe('files page', () => {
	beforeEach(() => {
		visit('/files');
		session.connect(() => me(FILE_USER));
	});

	it('lists files with their details and a download link', () => {
		render(FilesPage, page([storedFile()]));

		const [row] = screen.getAllByRole('listitem');
		expect(row).toHaveTextContent('report.pdf');
		expect(row).toHaveTextContent(formatBytes(1536n));
		expect(row).toHaveTextContent('application/pdf');
		const download = within(row!).getByRole('link', {
			name: t('files-download-label', { name: 'report.pdf' })
		});
		expect(download).toHaveAttribute('href', contentUrl(storedFile().id));
		expect(download).toHaveAttribute('download', 'report.pdf');
		expect(screen.queryByRole('meter')).not.toBeInTheDocument();
		expect(screen.queryByRole('group', { name: t('files-scope-label') })).not.toBeInTheDocument();
	});

	it('shows how full the storage is when there is a quota', () => {
		render(FilesPage, page([storedFile()], { quota: 1024n * 1024n }));

		expect(
			screen.getByText(t('files-usage', { used: formatBytes(1536n), quota: formatBytes(1048576n) }))
		).toBeInTheDocument();
		const meter = screen.getByRole('meter', { name: t('files-usage-label') });
		expect(meter).toHaveAttribute('aria-valuenow', '1536');
		expect(meter).toHaveAttribute('aria-valuemax', '1048576');
	});

	it('uploads the chosen files and refreshes the list', async () => {
		const user = userEvent.setup();
		const stored = storedFile(undefined, { name: 'q3.pdf' });
		const fetchFn = mockFetch(reply(StoredFileSchema, stored, 201));
		vi.stubGlobal('fetch', fetchFn);
		const success = vi.spyOn(toast, 'success');
		render(FilesPage, page([storedFile()]));

		const file = picked('q3.pdf');
		await user.upload(picker(), file);

		await vi.waitFor(() => expect(navigation.invalidate).toHaveBeenCalledWith('app:files'));
		expect(success).toHaveBeenCalledWith(t('files-uploaded', { name: 'q3.pdf' }));
		const [url, init] = callOf(fetchFn);
		expect(url).toBe('/api/v1/files?name=q3.pdf');
		expect(init.method).toBe('POST');
		expect(init.body).toBe(file);
		expect(picker().value).toBe('');
		expect(screen.queryByText(t('files-uploading', { name: 'q3.pdf' }))).not.toBeInTheDocument();
	});

	it('shows an upload on its way', async () => {
		const user = userEvent.setup();
		let answer: (response: Response) => void = () => {};
		vi.stubGlobal(
			'fetch',
			vi.fn<typeof fetch>(() => new Promise((resolve) => (answer = resolve)))
		);
		render(FilesPage, page([storedFile()]));

		await user.upload(picker(), picked('big.pdf'));

		expect(await screen.findByText(t('files-uploading', { name: 'big.pdf' }))).toBeInTheDocument();
		answer(reply(StoredFileSchema, storedFile(undefined, { name: 'big.pdf' }), 201));
		await vi.waitFor(() =>
			expect(screen.queryByText(t('files-uploading', { name: 'big.pdf' }))).not.toBeInTheDocument()
		);
	});

	it('refuses a file over the limit without sending it', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(reply(StoredFileSchema, storedFile(), 201));
		vi.stubGlobal('fetch', fetchFn);
		const failure = vi.spyOn(toast, 'error');
		render(FilesPage, page([storedFile()]));

		await user.upload(picker(), picked('huge.iso', MAX_FILE_SIZE + 1));

		expect(failure).toHaveBeenCalledWith(
			t('files-too-large', { name: 'huge.iso', max: formatBytes(MAX_FILE_SIZE) })
		);
		expect(fetchFn).not.toHaveBeenCalled();
		expect(navigation.invalidate).not.toHaveBeenCalled();
	});

	it('reports what the server refused', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(problem(409, 'file_quota_exceeded', { detail: 'there is not enough space' }))
		);
		const failure = vi.spyOn(toast, 'error');
		render(FilesPage, page([storedFile()]));

		await user.upload(picker(), picked('q3.pdf'));

		await vi.waitFor(() =>
			expect(failure).toHaveBeenCalledWith(
				t('files-upload-failed', { name: 'q3.pdf', reason: 'there is not enough space' })
			)
		);
		expect(navigation.invalidate).not.toHaveBeenCalled();
	});

	it('says which field the server refused, not just that the request was invalid', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(
				problem(422, 'validation_failed', {
					detail: 'the request is invalid',
					errors: [
						{ field: 'contentType', code: 'invalid_content_type', message: 'is not a media type' }
					]
				})
			)
		);
		const failure = vi.spyOn(toast, 'error');
		render(FilesPage, page([storedFile()]));

		await user.upload(picker(), picked('q3.pdf'));

		await vi.waitFor(() =>
			expect(failure).toHaveBeenCalledWith(
				t('files-upload-failed', { name: 'q3.pdf', reason: 'is not a media type' })
			)
		);
	});

	it('uploads a name the server would refuse under one it accepts', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(
			reply(StoredFileSchema, storedFile(undefined, { name: 'a_b.pdf' }), 201)
		);
		vi.stubGlobal('fetch', fetchFn);
		render(FilesPage, page([storedFile()]));

		await user.upload(picker(), picked('a\\b.pdf'));

		await vi.waitFor(() => expect(fetchFn).toHaveBeenCalled());
		expect(callOf(fetchFn)[0]).toBe('/api/v1/files?name=a_b.pdf');
	});

	it('uploads from a later page back to the first', async () => {
		const user = userEvent.setup();
		vi.stubGlobal('fetch', mockFetch(reply(StoredFileSchema, storedFile(), 201)));
		visit('/files?after=c1');
		render(FilesPage, page([storedFile()], { after: 'c1' }));

		await user.upload(picker(), picked('q3.pdf'));

		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenLastCalledWith('/files', expect.anything())
		);
	});

	it('asks before deleting', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(noContent(), problem(404, 'not_found'));
		vi.stubGlobal('fetch', fetchFn);
		const success = vi.spyOn(toast, 'success');
		const failure = vi.spyOn(toast, 'error');
		render(FilesPage, page([storedFile()]));

		const remove = () =>
			screen.getByRole('button', { name: t('files-delete-label', { name: 'report.pdf' }) });
		await user.click(remove());
		const dialog = await screen.findByRole('alertdialog');
		expect(dialog).toHaveTextContent(t('files-delete-description', { name: 'report.pdf' }));
		expect(fetchFn).not.toHaveBeenCalled();

		await user.click(within(dialog).getByRole('button', { name: t('files-delete-confirm') }));
		await vi.waitFor(() => expect(callOf(fetchFn)[1].method).toBe('DELETE'));
		expect(callOf(fetchFn)[0]).toBe(`/api/v1/files/${storedFile().id}`);
		expect(success).toHaveBeenCalledWith(t('files-deleted', { name: 'report.pdf' }));
		expect(navigation.invalidate).toHaveBeenCalledWith('app:files');

		await vi.waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
		document.body.removeAttribute('style');
		await user.click(remove());
		await user.click(
			within(await screen.findByRole('alertdialog')).getByRole('button', {
				name: t('files-delete-confirm')
			})
		);
		await vi.waitFor(() => expect(failure).toHaveBeenCalled());
		expect(navigation.invalidate).toHaveBeenCalledOnce();
	});

	it('renames a file', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(reply(StoredFileSchema, storedFile(undefined, { name: 'q3.pdf' })))
		);
		const success = vi.spyOn(toast, 'success');
		render(FilesPage, page([storedFile()]));

		await user.click(
			screen.getByRole('button', { name: t('files-rename-label', { name: 'report.pdf' }) })
		);
		const name = await screen.findByLabelText(t('file-field-name'));
		expect(name).toHaveValue('report.pdf');
		await user.clear(name);
		await user.type(name, 'q3.pdf');
		await user.click(screen.getByRole('button', { name: t('file-rename-save') }));

		await vi.waitFor(() => expect(navigation.invalidate).toHaveBeenCalledWith('app:files'));
		expect(success).toHaveBeenCalledWith(t('files-renamed', { name: 'q3.pdf' }));
	});

	it('offers only what the user may do', () => {
		session.connect(() => me([Permission.FILES_READ]));
		render(
			FilesPage,
			page([storedFile(), storedFile(OTHER_ID, { id: 'f2', name: 'theirs.txt' })], {
				permissions: [Permission.FILES_READ]
			})
		);

		expect(screen.queryByRole('button', { name: t('files-upload') })).not.toBeInTheDocument();
		expect(screen.queryByLabelText(t('files-upload-label'))).not.toBeInTheDocument();
		expect(
			screen.queryByRole('button', { name: new RegExp(t('files-rename')) })
		).not.toBeInTheDocument();
		expect(
			screen.queryByRole('button', { name: new RegExp(t('files-delete')) })
		).not.toBeInTheDocument();
		const [, theirs] = screen.getAllByRole('listitem');
		expect(within(theirs!).getByText(t('files-others'))).toBeInTheDocument();
	});

	it('lets managers switch whose files to show, and starts empty', async () => {
		const user = userEvent.setup();
		session.connect(() => me(MANAGER));
		render(FilesPage, page([], { permissions: MANAGER }));

		expect(screen.getByText(t('files-empty-title'))).toBeInTheDocument();
		expect(screen.getByRole('button', { name: t('files-upload-first') })).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: t('files-scope-all') }));
		expect(navigation.goto).toHaveBeenLastCalledWith('/files?scope=all', expect.anything());
		await user.click(screen.getByRole('button', { name: t('files-scope-mine') }));
		expect(navigation.goto).toHaveBeenLastCalledWith('/files?scope=mine', expect.anything());
	});

	it('pages through the files', async () => {
		const user = userEvent.setup();
		render(FilesPage, page([storedFile()], { nextCursor: 'c1' }));

		await user.click(screen.getByRole('button', { name: t('pagination-next') }));
		expect(navigation.goto).toHaveBeenLastCalledWith('/files?after=c1', expect.anything());
	});
});

describe('file rename dialog', () => {
	it('validates like the server, then saves', async () => {
		const user = userEvent.setup();
		const renamed = storedFile(undefined, { name: 'final.pdf' });
		const fetchFn = mockFetch(reply(StoredFileSchema, renamed));
		vi.stubGlobal('fetch', fetchFn);
		const onsaved = vi.fn();
		render(FileRenameDialog, { open: true, file: storedFile(), onsaved });

		const name = screen.getByLabelText(t('file-field-name'));
		await user.clear(name);
		await user.click(screen.getByRole('button', { name: t('file-rename-save') }));
		expect(name).toHaveAccessibleDescription(t('validation-required'));
		await user.type(name, 'a/b.pdf');
		await user.click(screen.getByRole('button', { name: t('file-rename-save') }));
		expect(name).toHaveAccessibleDescription(t('file-name-invalid'));
		expect(fetchFn).not.toHaveBeenCalled();

		await user.clear(name);
		await user.type(name, 'final.pdf');
		await user.click(screen.getByRole('button', { name: t('file-rename-save') }));

		await vi.waitFor(() => expect(onsaved).toHaveBeenCalledWith(renamed));
		const [url, init] = callOf(fetchFn);
		expect(url).toBe(`/api/v1/files/${storedFile().id}`);
		expect(init.method).toBe('PATCH');
		expect(sent(fetchFn, UpdateFileRequestSchema)).toEqual(
			create(UpdateFileRequestSchema, { name: 'final.pdf' })
		);
	});

	it('shows the server field errors and cancels', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(
				problem(422, 'validation_failed', {
					errors: [{ field: 'name', code: 'too_long', message: 'must be shorter' }]
				})
			)
		);
		render(FileRenameDialog, { open: true, file: storedFile(), onsaved: vi.fn() });

		await user.click(screen.getByRole('button', { name: t('file-rename-save') }));
		const name = screen.getByLabelText(t('file-field-name'));
		await vi.waitFor(() => expect(name).toHaveAccessibleDescription('must be shorter'));

		await user.click(screen.getByRole('button', { name: t('common-cancel') }));
		await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});
});
