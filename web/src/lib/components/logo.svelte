<script lang="ts">
	import BoxIcon from '@lucide/svelte/icons/box';
	import { site } from '$lib/helpers/site.svelte';
	import { cn } from '$lib/utils';

	let { class: className, withName = true }: { class?: string; withName?: boolean } = $props();
</script>

<span class={cn('flex items-center gap-2 font-semibold', className)}>
	<span
		class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-primary text-primary-foreground"
	>
		<BoxIcon class="size-4" />
	</span>
	{#if withName}<span class="truncate">{site.name}</span>{/if}
</span>
