<script lang="ts">
	import DownloadIcon from '@lucide/svelte/icons/download';
	import FileIcon from '@lucide/svelte/icons/file';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import UploadIcon from '@lucide/svelte/icons/upload';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { ApiError, api, contentUrl, errorMessage } from '$lib/api';
	import { filePolicy, session, tabSync } from '$lib/auth';
	import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
	import CursorPagination from '$lib/components/cursor-pagination.svelte';
	import FileRenameDialog from '$lib/components/file-rename-dialog.svelte';
	import PageHeading from '$lib/components/page-heading.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Spinner } from '$lib/components/ui/spinner';
	import { MAX_FILE_SIZE, uploadName } from '$lib/forms/validation';
	import { FILES } from '$lib/helpers/dependencies';
	import { formatBytes, formatRelative } from '$lib/helpers/format';
	import { CursorTrail } from '$lib/helpers/pagination';
	import { withQuery } from '$lib/helpers/url';
	import { t } from '$lib/i18n';
	import { Permission, type StoredFile } from '$lib/types/api';

	let { data } = $props();

	const me = $derived(data.me);
	const canUpload = $derived(session.can(filePolicy, 'create'));
	const canSeeAll = $derived(session.has(Permission.FILES_MANAGE));
	const quota = $derived(data.usage.quotaBytes);
	const used = $derived(data.usage.usedBytes);

	let picker = $state<HTMLInputElement | null>(null);
	let uploading = $state<{ id: number; name: string }[]>([]);
	let nextUpload = 0;
	let renaming = $state<StoredFile | null>(null);
	let renameOpen = $state(false);
	let deleting = $state<StoredFile | null>(null);
	let deleteOpen = $state(false);

	const trail = new CursorTrail();

	/** Sends each chosen file on its own; too large ones never leave the browser. */
	async function upload(chosen: File[]) {
		const results = await Promise.all(chosen.map(uploadOne));
		if (!results.includes(true)) return;
		if (data.after) {
			trail.reset();
			await navigate(null);
		} else {
			await tabSync.refresh(FILES);
		}
	}

	async function uploadOne(file: File): Promise<boolean> {
		if (file.size > MAX_FILE_SIZE) {
			toast.error(t('files-too-large', { name: file.name, max: formatBytes(MAX_FILE_SIZE) }));
			return false;
		}
		const entry = { id: nextUpload++, name: uploadName(file.name) };
		uploading = [...uploading, entry];
		try {
			const stored = await api.files.upload(file, entry.name);
			toast.success(t('files-uploaded', { name: stored.name }));
			return true;
		} catch (error) {
			// Several files may fail at once, so each notice names its file.
			const fieldError = error instanceof ApiError ? error.fieldErrors[0] : undefined;
			const reason = fieldError?.message ?? errorMessage(error);
			toast.error(t('files-upload-failed', { name: entry.name, reason }));
			return false;
		} finally {
			uploading = uploading.filter((other) => other.id !== entry.id);
		}
	}

	function picked(event: Event & { currentTarget: HTMLInputElement }) {
		const input = event.currentTarget;
		const chosen = [...(input.files ?? [])];
		// Cleared, so choosing the same file again uploads it again.
		input.value = '';
		void upload(chosen);
	}

	async function renamed(file: StoredFile) {
		toast.success(t('files-renamed', { name: file.name }));
		await tabSync.refresh(FILES);
	}

	async function confirmDelete() {
		const file = deleting;
		if (!file) return;
		try {
			await api.files.remove(file.id);
			toast.success(t('files-deleted', { name: file.name }));
			await tabSync.refresh(FILES);
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	function navigate(after: string | null, scope?: string) {
		// A same-site path built from the current URL.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		return goto(withQuery(page.url, { after, ...(scope ? { scope } : {}) }), {
			keepFocus: true,
			noScroll: false
		});
	}

	function next() {
		if (!data.files.nextCursor) return;
		void navigate(trail.next(data.after, data.files.nextCursor));
	}

	function previous() {
		void navigate(trail.previous(data.after));
	}

	function switchScope(scope: 'mine' | 'all') {
		trail.reset();
		void navigate(null, scope);
	}
</script>

<PageHeading title={t('files-title')} description={t('files-description')}>
	{#snippet actions()}
		{#if canUpload}
			<input
				bind:this={picker}
				type="file"
				multiple
				class="sr-only"
				tabindex="-1"
				aria-label={t('files-upload-label')}
				onchange={picked}
			/>
			<Button onclick={() => picker?.click()}>
				<UploadIcon data-icon="inline-start" />
				{t('files-upload')}
			</Button>
		{/if}
	{/snippet}
</PageHeading>

{#if quota !== undefined}
	<div class="flex flex-col gap-1.5">
		<p class="text-sm text-muted-foreground">
			{t('files-usage', { used: formatBytes(used), quota: formatBytes(quota) })}
		</p>
		<div
			class="h-1.5 overflow-hidden rounded-full bg-muted"
			role="meter"
			aria-label={t('files-usage-label')}
			aria-valuemin={0}
			aria-valuemax={Number(quota)}
			aria-valuenow={Number(used)}
		>
			<div
				class="h-full rounded-full bg-primary"
				style:width="{quota > 0n ? Math.min(100, (Number(used) / Number(quota)) * 100) : 100}%"
			></div>
		</div>
	</div>
{/if}

{#if canSeeAll}
	<div class="flex gap-2" role="group" aria-label={t('files-scope-label')}>
		<Button
			size="sm"
			variant={data.scope === 'mine' ? 'secondary' : 'ghost'}
			aria-pressed={data.scope === 'mine'}
			onclick={() => switchScope('mine')}
		>
			{t('files-scope-mine')}
		</Button>
		<Button
			size="sm"
			variant={data.scope === 'all' ? 'secondary' : 'ghost'}
			aria-pressed={data.scope === 'all'}
			onclick={() => switchScope('all')}
		>
			{t('files-scope-all')}
		</Button>
	</div>
{/if}

{#if uploading.length > 0}
	<ul class="flex flex-col gap-1" aria-live="polite">
		{#each uploading as entry (entry.id)}
			<li class="flex items-center gap-2 text-sm text-muted-foreground">
				<Spinner />
				{t('files-uploading', { name: entry.name })}
			</li>
		{/each}
	</ul>
{/if}

{#if data.files.items.length === 0}
	<Card.Root>
		<Card.Content class="flex flex-col items-center gap-3 py-10 text-center">
			<FolderOpenIcon class="size-8 text-muted-foreground" />
			<p class="font-medium">{t('files-empty-title')}</p>
			<p class="max-w-sm text-sm text-muted-foreground">{t('files-empty-description')}</p>
			{#if canUpload}
				<Button variant="outline" onclick={() => picker?.click()}>{t('files-upload-first')}</Button>
			{/if}
		</Card.Content>
	</Card.Root>
{:else}
	<Card.Root size="sm" class="py-0">
		<ul class="divide-y">
			{#each data.files.items as file (file.id)}
				{@const mine = file.ownerId === me.user.id}
				{@const canRename = session.can(filePolicy, 'update', file)}
				{@const canDelete = session.can(filePolicy, 'delete', file)}
				<li class="flex flex-wrap items-center gap-3 px-4 py-3">
					<FileIcon class="size-5 shrink-0 text-muted-foreground" />
					<div class="flex min-w-0 flex-1 flex-col">
						<span class="flex items-center gap-2">
							<span class="truncate font-medium">{file.name}</span>
							{#if !mine}<Badge variant="outline">{t('files-others')}</Badge>{/if}
						</span>
						<span class="truncate text-xs text-muted-foreground">
							{t('files-details', {
								size: formatBytes(file.size),
								type: file.contentType,
								when: formatRelative(file.createdAt)
							})}
						</span>
					</div>
					<div class="flex gap-1">
						<Button
							variant="ghost"
							size="sm"
							href={contentUrl(file.id)}
							download={file.name}
							aria-label={t('files-download-label', { name: file.name })}
						>
							<DownloadIcon data-icon="inline-start" />
							{t('files-download')}
						</Button>
						{#if canRename}
							<Button
								variant="ghost"
								size="sm"
								onclick={() => ((renaming = file), (renameOpen = true))}
								aria-label={t('files-rename-label', { name: file.name })}
							>
								<PencilIcon data-icon="inline-start" />
								{t('files-rename')}
							</Button>
						{/if}
						{#if canDelete}
							<Button
								variant="destructive"
								size="sm"
								onclick={() => ((deleting = file), (deleteOpen = true))}
								aria-label={t('files-delete-label', { name: file.name })}
							>
								<Trash2Icon data-icon="inline-start" />
								{t('files-delete')}
							</Button>
						{/if}
					</div>
				</li>
			{/each}
		</ul>
	</Card.Root>
{/if}

<CursorPagination
	label={t('files-pages-label')}
	hasPrevious={trail.hasPrevious(data.after)}
	hasNext={data.files.nextCursor !== undefined}
	onPrevious={previous}
	onNext={next}
/>

<FileRenameDialog bind:open={renameOpen} file={renaming} onsaved={renamed} />

<ConfirmDialog
	bind:open={deleteOpen}
	title={t('files-delete-title')}
	description={t('files-delete-description', { name: deleting?.name ?? '' })}
	confirmLabel={t('files-delete-confirm')}
	onconfirm={confirmDelete}
/>
