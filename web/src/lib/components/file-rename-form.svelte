<script lang="ts">
	import { api } from '$lib/api';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { MAX_FILE_NAME_LENGTH } from '$lib/forms/validation';
	import { t } from '$lib/i18n';
	import type { StoredFile } from '$lib/types/api';

	let {
		file,
		onsaved,
		oncancel
	}: {
		file: StoredFile;
		onsaved: (file: StoredFile) => void;
		oncancel: () => void;
	} = $props();

	// svelte-ignore state_referenced_locally
	let name = $state(file.name);
	const form = new FormState(['name']);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const saved = await form.submit(() => api.files.rename(file.id, name), {
			validation: validate({ name }, { name: rules.fileName })
		});
		if (saved) onsaved(saved);
	}
</script>

<form onsubmit={submit} novalidate class="flex flex-col gap-6">
	<Dialog.Header>
		<Dialog.Title>{t('file-rename-title')}</Dialog.Title>
		<Dialog.Description>{t('file-rename-description')}</Dialog.Description>
	</Dialog.Header>
	<Field.Group>
		<FormError message={form.error} />
		<FormField id="file-name" label={t('file-field-name')} error={form.fieldErrors.name}>
			<Input
				id="file-name"
				maxlength={MAX_FILE_NAME_LENGTH}
				required
				bind:value={name}
				oninput={() => form.clearField('name')}
				{...invalid('file-name', form.fieldErrors.name)}
			/>
		</FormField>
	</Field.Group>
	<Dialog.Footer>
		<Button type="button" variant="outline" onclick={oncancel}>{t('common-cancel')}</Button>
		<Button type="submit" disabled={form.pending}>
			{#if form.pending}<Spinner />{/if}
			{t('file-rename-save')}
		</Button>
	</Dialog.Footer>
</form>
