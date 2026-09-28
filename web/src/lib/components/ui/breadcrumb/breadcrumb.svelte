<script lang="ts">
	import { t } from '$lib/i18n';
	import type { WithElementRef } from '$lib/utils.js';
	import { cn } from '$lib/utils.js';
	import type { HTMLAttributes } from 'svelte/elements';

	let {
		ref = $bindable(null),
		class: className,
		children,
		...restProps
	}: WithElementRef<HTMLAttributes<HTMLElement>> = $props();
</script>

<nav
	bind:this={ref}
	data-slot="breadcrumb"
	aria-label={t('nav-breadcrumb')}
	class={cn(className)}
	{...restProps}
>
	{@render children?.()}
</nav>
