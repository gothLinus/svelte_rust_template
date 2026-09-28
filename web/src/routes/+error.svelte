<script lang="ts">
	import { dev } from '$app/environment';
	import { invalidateAll } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '$lib/components/ui/button';
	import RichText, { emphasize } from '$lib/components/rich-text.svelte';
	import { t } from '$lib/i18n';

	let retrying = $state(false);

	async function retry() {
		retrying = true;
		try {
			await invalidateAll();
		} finally {
			retrying = false;
		}
	}
</script>

<svelte:head><title>{page.status}</title></svelte:head>

<main class="mx-auto flex min-h-svh max-w-xl flex-col justify-center gap-3 p-4 sm:p-6">
	<p class="text-sm text-muted-foreground">{page.status}</p>
	<h1 class="text-2xl font-semibold">{page.error?.message ?? t('app-error-fallback')}</h1>
	{#if dev && page.status >= 500}
		<p class="text-sm text-muted-foreground">
			<RichText text={t('app-error-dev-hint', { command: emphasize('just dev') })} tag="code" />
		</p>
	{/if}
	<div class="flex gap-2">
		{#if page.status >= 500}
			<Button size="sm" disabled={retrying} onclick={retry}>{t('app-error-retry')}</Button>
		{/if}
		<Button href={resolve('/')} variant="outline" size="sm">{t('app-error-back-to-start')}</Button>
	</div>
</main>
