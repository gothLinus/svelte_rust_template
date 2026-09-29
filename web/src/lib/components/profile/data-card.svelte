<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { api, errorMessage } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Spinner } from '$lib/components/ui/spinner';
	import { t } from '$lib/i18n';

	let pending = $state(false);

	async function download() {
		pending = true;
		try {
			const file = await api.me.exportData();
			const url = URL.createObjectURL(file);
			const link = document.createElement('a');
			link.href = url;
			link.download = 'personal-data.json';
			link.click();
			URL.revokeObjectURL(url);
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			pending = false;
		}
	}
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('profile-data-title')}</Card.Title>
		<Card.Description>{t('profile-data-description')}</Card.Description>
	</Card.Header>
	<Card.Footer class="justify-end">
		<Button variant="outline" size="sm" disabled={pending} onclick={download}>
			{#if pending}<Spinner />{/if}
			{t('profile-data-download')}
		</Button>
	</Card.Footer>
</Card.Root>
