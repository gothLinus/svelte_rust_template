<script lang="ts">
	import { Separator } from '$lib/components/ui/separator';
	import { t } from '$lib/i18n';

	let { label }: { label?: string } = $props();
</script>

<div class="flex items-center gap-3 text-xs text-muted-foreground" role="separator">
	<Separator class="flex-1" />
	<span>{label ?? t('signin-or')}</span>
	<Separator class="flex-1" />
</div>
