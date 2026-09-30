<script lang="ts" module>
	// Private-use characters that cannot occur in a translation or in typed text.
	const OPEN = '\uE000';
	const CLOSE = '\uE001';

	/**
	 * Marks a placeholder value so `RichText` shows it emphasized:
	 * `<RichText text={t(id, { email: emphasize(email) })} />`. The sentence stays one
	 * message, so translators can move the value wherever their language wants it.
	 */
	export function emphasize(value: string): string {
		return `${OPEN}${value.replaceAll(OPEN, '').replaceAll(CLOSE, '')}${CLOSE}`;
	}
</script>

<script lang="ts">
	let { text, tag = 'strong' }: { text: string; tag?: 'strong' | 'code' } = $props();

	const pieces = $derived(text.split(new RegExp(`[${OPEN}${CLOSE}]`)));
</script>

{#each pieces as piece, index (index)}{#if index % 2 === 1}<svelte:element this={tag}
			>{piece}</svelte:element
		>{:else}{piece}{/if}{/each}
