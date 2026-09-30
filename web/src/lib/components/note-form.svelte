<script lang="ts">
	import { api } from '$lib/api';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { Textarea } from '$lib/components/ui/textarea';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { t } from '$lib/i18n';
	import { MAX_NOTE_BODY_LENGTH, MAX_NOTE_TITLE_LENGTH } from '$lib/forms/validation';
	import type { Note } from '$lib/types/api';

	/**
	 * The inside of `NoteDialog`. The dialog mounts it each time it opens, so the fields
	 * start from `note` (or blank) without copying state around.
	 */
	let {
		note,
		onsaved,
		oncancel
	}: {
		note: Note | null;
		onsaved: (note: Note, created: boolean) => void;
		oncancel: () => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	let title = $state(note?.title ?? '');
	// svelte-ignore state_referenced_locally
	let body = $state(note?.body ?? '');
	const form = new FormState(['title', 'body']);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const editing = note;
		const saved = await form.submit(
			() =>
				editing ? api.notes.update(editing.id, { title, body }) : api.notes.create({ title, body }),
			{ validation: validate({ title, body }, { title: rules.noteTitle, body: rules.noteBody }) }
		);
		if (saved) onsaved(saved, !editing);
	}
</script>

<form onsubmit={submit} novalidate class="flex flex-col gap-6">
	<Dialog.Header>
		<Dialog.Title>{note ? t('note-title-edit') : t('note-title-new')}</Dialog.Title>
		<Dialog.Description>
			{note ? t('note-description-edit') : t('note-description-new')}
		</Dialog.Description>
	</Dialog.Header>
	<Field.Group>
		<FormError message={form.error} />
		<FormField id="note-title" label={t('note-field-title')} error={form.fieldErrors.title}>
			<Input
				id="note-title"
				maxlength={MAX_NOTE_TITLE_LENGTH}
				required
				bind:value={title}
				oninput={() => form.clearField('title')}
				{...invalid('note-title', form.fieldErrors.title)}
			/>
		</FormField>
		<FormField id="note-body" label={t('note-field-text')} error={form.fieldErrors.body}>
			<Textarea
				id="note-body"
				rows={6}
				maxlength={MAX_NOTE_BODY_LENGTH}
				bind:value={body}
				oninput={() => form.clearField('body')}
				{...invalid('note-body', form.fieldErrors.body)}
			/>
		</FormField>
	</Field.Group>
	<Dialog.Footer>
		<Button type="button" variant="outline" onclick={oncancel}>{t('common-cancel')}</Button>
		<Button type="submit" disabled={form.pending}>
			{#if form.pending}<Spinner />{/if}
			{note ? t('note-save') : t('note-create')}
		</Button>
	</Dialog.Footer>
</form>
