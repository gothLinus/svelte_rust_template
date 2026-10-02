<script lang="ts">
	import CopyIcon from '@lucide/svelte/icons/copy';
	import NotebookPenIcon from '@lucide/svelte/icons/notebook-pen';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import { ApiError, STALE, api, errorMessage } from '$lib/api';
	import { notePolicy, session, tabSync } from '$lib/auth';
	import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
	import CursorPagination from '$lib/components/cursor-pagination.svelte';
	import NoteDialog from '$lib/components/note-dialog.svelte';
	import PageHeading from '$lib/components/page-heading.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { type Note, Permission } from '$lib/types/api';
	import { NOTES } from '$lib/helpers/dependencies';
	import { excerpt, formatRelative } from '$lib/helpers/format';
	import { t } from '$lib/i18n';
	import { CursorTrail } from '$lib/helpers/pagination';
	import { withQuery } from '$lib/helpers/url';

	let { data } = $props();

	const me = $derived(data.me);
	const canCreate = $derived(session.can(notePolicy, 'create'));
	const canSeeAll = $derived(session.has(Permission.NOTES_MANAGE));

	let dialogOpen = $state(false);
	let editing = $state<Note | null>(null);
	let deleting = $state<Note | null>(null);
	let deleteOpen = $state(false);

	const trail = new CursorTrail();

	function openCreate() {
		editing = null;
		dialogOpen = true;
	}

	function openEdit(note: Note) {
		editing = note;
		dialogOpen = true;
	}

	async function saved(note: Note, created: boolean) {
		toast.success(created ? t('notes-created', { title: note.title }) : t('notes-saved'));
		if (created && data.after) {
			trail.reset();
			await navigate(null);
		} else {
			await tabSync.refresh(NOTES);
		}
	}

	// The server authorizes the copy like a create and the source like a read; every
	// listed note is readable, so `canCreate` decides.
	async function duplicate(note: Note) {
		try {
			const copy = await api.notes.duplicate(note.id);
			await saved(copy, true);
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	async function confirmDelete() {
		const note = deleting;
		if (!note) return;
		try {
			await api.notes.remove(note.id, note.version);
			toast.success(t('notes-deleted', { title: note.title }));
			await tabSync.refresh(NOTES);
		} catch (error) {
			toast.error(errorMessage(error));
			// A `stale` note changed since the list was loaded: show the current one.
			if (error instanceof ApiError && error.code === STALE) await tabSync.refresh(NOTES);
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
		if (!data.notes.nextCursor) return;
		void navigate(trail.next(data.after, data.notes.nextCursor));
	}

	function previous() {
		void navigate(trail.previous(data.after));
	}

	function switchScope(scope: 'mine' | 'all') {
		trail.reset();
		void navigate(null, scope === 'all' ? 'all' : 'mine');
	}
</script>

<PageHeading title={t('notes-title')} description={t('notes-description')}>
	{#snippet actions()}
		{#if canCreate}
			<Button onclick={openCreate}><PlusIcon data-icon="inline-start" /> {t('notes-new')}</Button>
		{/if}
	{/snippet}
</PageHeading>

{#if canSeeAll}
	<div class="flex gap-2" role="group" aria-label={t('notes-scope-label')}>
		<Button
			size="sm"
			variant={data.scope === 'mine' ? 'secondary' : 'ghost'}
			aria-pressed={data.scope === 'mine'}
			onclick={() => switchScope('mine')}
		>
			{t('notes-scope-mine')}
		</Button>
		<Button
			size="sm"
			variant={data.scope === 'all' ? 'secondary' : 'ghost'}
			aria-pressed={data.scope === 'all'}
			onclick={() => switchScope('all')}
		>
			{t('notes-scope-all')}
		</Button>
	</div>
{/if}

{#if data.notes.items.length === 0}
	<Card.Root>
		<Card.Content class="flex flex-col items-center gap-3 py-10 text-center">
			<NotebookPenIcon class="size-8 text-muted-foreground" />
			<p class="font-medium">{t('notes-empty-title')}</p>
			<p class="max-w-sm text-sm text-muted-foreground">
				{t('notes-empty-description')}
			</p>
			{#if canCreate}<Button variant="outline" onclick={openCreate}
					>{t('notes-create-first')}</Button
				>{/if}
		</Card.Content>
	</Card.Root>
{:else}
	<ul class="grid gap-3 sm:grid-cols-2">
		{#each data.notes.items as note (note.id)}
			{@const mine = note.ownerId === me.user.id}
			{@const canUpdate = session.can(notePolicy, 'update', note)}
			{@const canDelete = session.can(notePolicy, 'delete', note)}
			<li>
				<Card.Root size="sm" class="h-full">
					<Card.Header>
						<Card.Title level={2} class="truncate">{note.title}</Card.Title>
						<Card.Description class="flex items-center gap-2">
							<span>{t('notes-updated', { when: formatRelative(note.updatedAt) })}</span>
							{#if !mine}<Badge variant="outline">{t('notes-others')}</Badge>{/if}
						</Card.Description>
					</Card.Header>
					{#if note.body}
						<Card.Content>
							<p class="text-sm break-words text-muted-foreground">{excerpt(note.body)}</p>
						</Card.Content>
					{/if}
					{#if canUpdate || canCreate || canDelete}
						<Card.Footer class="mt-auto gap-2">
							{#if canUpdate}
								<Button
									variant="ghost"
									size="sm"
									onclick={() => openEdit(note)}
									aria-label={t('notes-edit-label', { title: note.title })}
								>
									<PencilIcon data-icon="inline-start" />
									{t('notes-edit')}
								</Button>
							{/if}
							{#if canCreate}
								<Button
									variant="ghost"
									size="sm"
									onclick={() => duplicate(note)}
									aria-label={t('notes-duplicate-label', { title: note.title })}
								>
									<CopyIcon data-icon="inline-start" />
									{t('notes-duplicate')}
								</Button>
							{/if}
							{#if canDelete}
								<Button
									variant="destructive"
									size="sm"
									onclick={() => ((deleting = note), (deleteOpen = true))}
									aria-label={t('notes-delete-label', { title: note.title })}
								>
									<Trash2Icon data-icon="inline-start" />
									{t('notes-delete')}
								</Button>
							{/if}
						</Card.Footer>
					{/if}
				</Card.Root>
			</li>
		{/each}
	</ul>
{/if}

<CursorPagination
	label={t('notes-pages-label')}
	hasPrevious={trail.hasPrevious(data.after)}
	hasNext={data.notes.nextCursor !== undefined}
	onPrevious={previous}
	onNext={next}
/>

<NoteDialog bind:open={dialogOpen} note={editing} onsaved={saved} />

<ConfirmDialog
	bind:open={deleteOpen}
	title={t('notes-delete-title')}
	description={t('notes-delete-description', { title: deleting?.title ?? '' })}
	confirmLabel={t('notes-delete-confirm')}
	onconfirm={confirmDelete}
/>
