<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { Button } from '$lib/components/ui/button';
	import { t } from '$lib/i18n';
</script>

<div class="flex flex-1 flex-col items-start justify-center gap-3 py-16">
	<p class="text-sm text-muted-foreground">{page.status}</p>
	<h1 class="text-2xl font-semibold">{page.error?.message ?? t('app-error-fallback')}</h1>
	<Button href={resolve('/dashboard')} variant="outline" size="sm"
		>{t('app-error-back-to-dashboard')}</Button
	>
</div>
