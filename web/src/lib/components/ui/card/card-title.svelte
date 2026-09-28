<script lang="ts">
	import { cn, type WithElementRef } from '$lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	let {
		ref = $bindable(null),
		class: className,
		level,
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLElement>> & {
		/** Renders an `<h1>`…`<h6>` for cards that start a section, instead of a `<div>`. */
		level?: 1 | 2 | 3 | 4 | 5 | 6;
	} = $props();
</script>

<svelte:element
	this={level ? `h${level}` : 'div'}
	bind:this={ref}
	data-slot="card-title"
	class={cn('text-base font-medium', className)}
	{...restProps}
>
	{@render children?.()}
</svelte:element>
