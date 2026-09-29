<script lang="ts">
	import CopyIcon from '@lucide/svelte/icons/copy';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { toast } from 'svelte-sonner';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import { site } from '$lib/helpers/site.svelte';
	import { t } from '$lib/i18n';

	/**
	 * Shows freshly generated recovery codes once, with ways to keep them. Only "I saved
	 * them" closes it: Escape or a click outside would lose codes that are never shown again.
	 */
	let { codes = $bindable() }: { codes: string[] | null } = $props();

	const text = $derived((codes ?? []).join('\n'));

	async function copy() {
		try {
			await navigator.clipboard.writeText(text);
			toast.success(t('security-recovery-copied'));
		} catch {
			toast.error(t('security-recovery-copy-failed'));
		}
	}

	function download() {
		const blob = new Blob(
			[`${t('security-recovery-file-title', { app: site.name })}\n\n${text}\n`],
			{ type: 'text/plain' }
		);
		const url = URL.createObjectURL(blob);
		const link = document.createElement('a');
		link.href = url;
		link.download = `${site.name.toLowerCase()}-recovery-codes.txt`;
		link.click();
		URL.revokeObjectURL(url);
	}
</script>

<Dialog.Root open={codes !== null}>
	<Dialog.Content
		showCloseButton={false}
		escapeKeydownBehavior="ignore"
		interactOutsideBehavior="ignore"
	>
		<Dialog.Header>
			<Dialog.Title>{t('security-recovery-dialog-title')}</Dialog.Title>
			<Dialog.Description>
				{t('security-recovery-dialog-description')}
			</Dialog.Description>
		</Dialog.Header>
		<ul
			class="grid grid-cols-2 gap-x-6 gap-y-1 rounded-2xl bg-muted p-4 font-mono text-sm"
			aria-label={t('security-recovery-list-label')}
		>
			{#each codes ?? [] as code (code)}<li>{code}</li>{/each}
		</ul>
		<Dialog.Footer>
			<Button variant="outline" onclick={copy}><CopyIcon />{t('security-recovery-copy')}</Button>
			<Button variant="outline" onclick={download}
				><DownloadIcon />{t('security-recovery-download')}</Button
			>
			<Button onclick={() => (codes = null)}>
				{t('security-recovery-saved')}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
