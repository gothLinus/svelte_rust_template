<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { page } from '$app/state';
	import { api, errorMessage } from '$lib/api';
	import { lifetimes } from '$lib/auth';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Spinner } from '$lib/components/ui/spinner';
	import { t } from '$lib/i18n';

	/**
	 * Changing the password goes through a link mailed to the account's address, so a
	 * session left open somewhere cannot be used to lock the owner out.
	 */
	let { email, hasPassword }: { email: string; hasPassword: boolean } = $props();

	let pending = $state(false);

	async function sendLink() {
		pending = true;
		try {
			await api.me.requestPasswordChange();
			toast.success(
				t('security-password-link-sent', {
					email,
					minutes: lifetimes(page.data.methods).passwordResetMinutes
				})
			);
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			pending = false;
		}
	}
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('security-password-title')}</Card.Title>
		<Card.Description>
			{#if hasPassword}
				{t('security-password-description')}
			{:else}
				{t('security-password-description-none')}
			{/if}
		</Card.Description>
	</Card.Header>
	<Card.Footer class="justify-end">
		<Button variant="outline" size="sm" disabled={pending} onclick={sendLink}>
			{#if pending}<Spinner />{/if}
			{hasPassword ? t('security-password-change') : t('security-password-set')}
		</Button>
	</Card.Footer>
</Card.Root>
