<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { EVERYTHING, tabSync } from '$lib/auth';
	import { resolve } from '$app/paths';
	import { api, errorMessage } from '$lib/api';
	import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
	import SessionItem from '$lib/components/security/session-item.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import type { Session } from '$lib/types/api';
	import { SESSIONS } from '$lib/helpers/dependencies';
	import { describeUserAgent } from '$lib/helpers/format';
	import { t } from '$lib/i18n';

	let { sessions }: { sessions: Session[] } = $props();

	let busy = $state(false);

	async function revoke(session: Session) {
		busy = true;
		try {
			await api.me.revokeSession(session.id);
			await tabSync.refresh(SESSIONS);
			toast.success(
				t('security-sessions-signed-out', { device: describeUserAgent(session.userAgent) })
			);
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			busy = false;
		}
	}

	let everywhereOpen = $state(false);

	async function signOutEverywhere() {
		try {
			await api.auth.logoutAll();
		} catch (error) {
			toast.error(errorMessage(error));
			return;
		}
		toast.success(t('security-sessions-everywhere-done'));
		tabSync.announce(EVERYTHING);
		await goto(resolve('/login'), { invalidateAll: true });
	}
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('security-sessions-title')}</Card.Title>
		<Card.Description>
			{t('security-sessions-description')}
		</Card.Description>
	</Card.Header>
	<Card.Content>
		<ul class="divide-y">
			{#each sessions as session (session.id)}
				<SessionItem {session} {busy} onrevoke={revoke} />
			{/each}
		</ul>
	</Card.Content>
	<Card.Footer class="justify-end">
		<Button variant="outline" size="sm" disabled={busy} onclick={() => (everywhereOpen = true)}>
			{t('security-sessions-everywhere')}
		</Button>
	</Card.Footer>
</Card.Root>

<ConfirmDialog
	bind:open={everywhereOpen}
	title={t('security-sessions-everywhere-title')}
	description={t('security-sessions-everywhere-description')}
	confirmLabel={t('security-sessions-everywhere')}
	onconfirm={signOutEverywhere}
/>
