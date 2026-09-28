<script lang="ts">
	import CloudOffIcon from '@lucide/svelte/icons/cloud-off';
	import { invalidate } from '$app/navigation';
	import { SESSION } from '$lib/auth';
	import * as Alert from '$lib/components/ui/alert';
	import { Button } from '$lib/components/ui/button';
	import { t } from '$lib/i18n';

	let { message }: { message: string } = $props();

	let retrying = $state(false);

	async function retry() {
		retrying = true;
		try {
			await invalidate(SESSION);
		} finally {
			retrying = false;
		}
	}
</script>

<Alert.Root variant="destructive" class="rounded-none border-x-0 border-t-0">
	<CloudOffIcon />
	<Alert.Title>{t('session-error-title')}</Alert.Title>
	<Alert.Description>
		<p>{message}</p>
		<Button variant="link" size="inline" disabled={retrying} onclick={retry}>
			{t('session-error-retry')}
		</Button>
	</Alert.Description>
</Alert.Root>
