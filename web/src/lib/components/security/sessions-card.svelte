<script lang="ts">
	import MonitorIcon from '@lucide/svelte/icons/monitor';
	import SmartphoneIcon from '@lucide/svelte/icons/smartphone';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { EVERYTHING, tabSync } from '$lib/auth';
	import { resolve } from '$app/paths';
	import { api, errorMessage } from '$lib/api';
	import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import type { Session } from '$lib/types/api';
	import { SESSIONS } from '$lib/helpers/dependencies';
	import { describeUserAgent, formatDateTime, formatRelative } from '$lib/helpers/format';
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

	const isMobile = (session: Session) => /Mobi|Android|iPhone|iPad/.test(session.userAgent ?? '');
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
				{@const Icon = isMobile(session) ? SmartphoneIcon : MonitorIcon}
				<li class="flex items-center gap-3 py-2.5 first:pt-0 last:pb-0">
					<div class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-muted">
						<Icon class="size-4 text-muted-foreground" />
					</div>
					<div class="flex min-w-0 flex-1 flex-col">
						<div class="flex items-center gap-2">
							<span class="truncate text-sm font-medium">
								{describeUserAgent(session.userAgent)}
							</span>
							{#if session.current}<Badge variant="secondary"
									>{t('security-sessions-current')}</Badge
								>{/if}
						</div>
						<span class="flex flex-col text-xs text-muted-foreground sm:flex-row sm:gap-1">
							<span
								>{t('security-sessions-active', { when: formatRelative(session.lastSeenAt) })}</span
							>
							<span>
								<span class="hidden sm:inline">·</span>
								{t('security-sessions-signed-in', { when: formatDateTime(session.createdAt) })}
							</span>
							{#if session.ip}
								<span><span class="hidden sm:inline">·</span> {session.ip}</span>
							{/if}
						</span>
					</div>
					{#if !session.current}
						<Button
							variant="ghost"
							size="sm"
							disabled={busy}
							aria-label={t('security-sessions-sign-out-label', {
								device: describeUserAgent(session.userAgent),
								active: formatRelative(session.lastSeenAt)
							})}
							onclick={() => revoke(session)}
						>
							{t('security-sessions-sign-out')}
						</Button>
					{/if}
				</li>
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
