<script lang="ts">
	import { resolve } from '$app/paths';
	import Logo from '$lib/components/logo.svelte';
	import ThemeToggle from '$lib/components/theme-toggle.svelte';

	let { children } = $props();
</script>

<div class="flex min-h-svh flex-col bg-muted/40">
	<header class="flex items-center justify-between p-4 sm:p-6">
		<a href={resolve('/')} class="rounded-xl">
			<Logo />
		</a>
		<ThemeToggle />
	</header>
	<main
		class="flex flex-1 items-start justify-center px-4 pt-2 pb-[calc(--spacing(10)+env(safe-area-inset-bottom))] sm:items-center sm:pt-0 sm:pb-16"
	>
		<div class="w-full max-w-sm">
			{@render children()}
		</div>
	</main>
</div>
