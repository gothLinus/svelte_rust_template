<script lang="ts">
	import type { Snippet } from 'svelte';
	import * as Field from '$lib/components/ui/field';

	/**
	 * A labelled form field with its description or error. Give the input `id` and spread
	 * `invalid(id, error)` on it (`invalid(id, error, { described: true })` with a
	 * `description`), so screen readers announce both.
	 */
	let {
		id,
		label,
		error,
		description,
		labelAction,
		children
	}: {
		id: string;
		label: string;
		error?: string;
		description?: string;
		labelAction?: Snippet;
		children: Snippet;
	} = $props();
</script>

<Field.Field data-invalid={error ? true : undefined}>
	{#if labelAction}
		<div class="flex items-center">
			<Field.Label for={id}>{label}</Field.Label>
			<div class="ml-auto">{@render labelAction()}</div>
		</div>
	{:else}
		<Field.Label for={id}>{label}</Field.Label>
	{/if}
	{@render children()}
	{#if error}
		<Field.Error id="{id}-error">{error}</Field.Error>
	{:else if description}
		<Field.Description id="{id}-description">{description}</Field.Description>
	{/if}
</Field.Field>
