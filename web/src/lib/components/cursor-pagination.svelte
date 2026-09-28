<script lang="ts">
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import { Button } from '$lib/components/ui/button';
	import { t } from '$lib/i18n';

	/**
	 * Previous/next buttons for keyset-paginated lists. The server only knows "the page after
	 * this cursor", so the list page remembers the cursors it came through (see `CursorTrail`).
	 */
	let {
		hasPrevious,
		hasNext,
		onPrevious,
		onNext,
		label
	}: {
		hasPrevious: boolean;
		hasNext: boolean;
		onPrevious: () => void;
		onNext: () => void;
		label?: string;
	} = $props();
</script>

{#if hasPrevious || hasNext}
	<nav aria-label={label ?? t('pagination-label')} class="flex justify-end gap-2">
		<Button variant="outline" size="sm" disabled={!hasPrevious} onclick={onPrevious}>
			<ChevronLeftIcon data-icon="inline-start" />
			{t('pagination-previous')}
		</Button>
		<Button variant="outline" size="sm" disabled={!hasNext} onclick={onNext}>
			{t('pagination-next')}
			<ChevronRightIcon data-icon="inline-end" />
		</Button>
	</nav>
{/if}
