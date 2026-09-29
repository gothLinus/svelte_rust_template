<script lang="ts">
	import MailWarningIcon from '@lucide/svelte/icons/mail-warning';
	import { toast } from 'svelte-sonner';
	import { api, errorMessage } from '$lib/api';
	import RichText, { emphasize } from '$lib/components/rich-text.svelte';
	import * as Alert from '$lib/components/ui/alert';
	import { Button } from '$lib/components/ui/button';
	import { t } from '$lib/i18n';

	let { email }: { email: string } = $props();

	let sending = $state(false);

	async function resend() {
		sending = true;
		try {
			await api.auth.resendVerification();
			toast.success(t('email-verification-resent', { email }));
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			sending = false;
		}
	}
</script>

<Alert.Root>
	<MailWarningIcon />
	<Alert.Title>{t('email-verification-title')}</Alert.Title>
	<Alert.Description>
		<p><RichText text={t('email-verification-body', { email: emphasize(email) })} /></p>
		<Button variant="link" size="inline" disabled={sending} onclick={resend}>
			{t('email-verification-resend')}
		</Button>
	</Alert.Description>
</Alert.Root>
