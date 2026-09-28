<script lang="ts">
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Button } from '$lib/components/ui/button';
	import { Spinner } from '$lib/components/ui/spinner';
	import { t } from '$lib/i18n';

	/**
	 * Asks before an action that cannot be taken back. Open it by setting `open`; it closes
	 * once `onconfirm` has finished. `onconfirm` reports its own errors, e.g. with a toast.
	 */
	let {
		open = $bindable(false),
		title,
		description,
		confirmLabel,
		destructive = true,
		onconfirm
	}: {
		open?: boolean;
		title: string;
		description: string;
		confirmLabel: string;
		destructive?: boolean;
		onconfirm: () => Promise<void>;
	} = $props();

	let pending = $state(false);

	async function confirm() {
		pending = true;
		try {
			await onconfirm();
		} finally {
			pending = false;
			open = false;
		}
	}
</script>

<AlertDialog.Root bind:open>
	<AlertDialog.Content>
		<AlertDialog.Header>
			<AlertDialog.Title>{title}</AlertDialog.Title>
			<AlertDialog.Description>{description}</AlertDialog.Description>
		</AlertDialog.Header>
		<AlertDialog.Footer>
			<AlertDialog.Cancel disabled={pending}>{t('common-cancel')}</AlertDialog.Cancel>
			<Button
				variant={destructive ? 'destructive' : 'default'}
				disabled={pending}
				onclick={confirm}
			>
				{#if pending}<Spinner />{/if}
				{confirmLabel}
			</Button>
		</AlertDialog.Footer>
	</AlertDialog.Content>
</AlertDialog.Root>
