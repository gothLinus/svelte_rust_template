<script lang="ts">
	import MonitorIcon from '@lucide/svelte/icons/monitor';
	import SmartphoneIcon from '@lucide/svelte/icons/smartphone';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import type { Session } from '$lib/types/api';
	import { describeUserAgent, formatDateTime, formatRelative } from '$lib/helpers/format';
	import { t } from '$lib/i18n';

	/**
	 * One signed-in device in a list of sessions, the user's own or (for admins) someone
	 * else's. Without `onrevoke`, or for the current session, there is no sign-out button.
	 */
	let {
		session,
		busy = false,
		onrevoke
	}: {
		session: Session;
		busy?: boolean;
		onrevoke?: (session: Session) => void;
	} = $props();

	const Icon = $derived(
		/Mobi|Android|iPhone|iPad/.test(session.userAgent ?? '') ? SmartphoneIcon : MonitorIcon
	);
</script>

<li class="flex items-center gap-3 py-2.5 first:pt-0 last:pb-0">
	<div class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-muted">
		<Icon class="size-4 text-muted-foreground" />
	</div>
	<div class="flex min-w-0 flex-1 flex-col">
		<div class="flex items-center gap-2">
			<span class="truncate text-sm font-medium">
				{describeUserAgent(session.userAgent)}
			</span>
			{#if session.current}<Badge variant="secondary">{t('security-sessions-current')}</Badge>{/if}
		</div>
		<span class="flex flex-col text-xs text-muted-foreground sm:flex-row sm:gap-1">
			<span>{t('security-sessions-active', { when: formatRelative(session.lastSeenAt) })}</span>
			<span>
				<span class="hidden sm:inline">·</span>
				{t('security-sessions-signed-in', { when: formatDateTime(session.createdAt) })}
			</span>
			{#if session.ip}
				<span><span class="hidden sm:inline">·</span> {session.ip}</span>
			{/if}
		</span>
	</div>
	{#if onrevoke && !session.current}
		<Button
			variant="ghost"
			size="sm"
			disabled={busy}
			aria-label={t('security-sessions-sign-out-label', {
				device: describeUserAgent(session.userAgent),
				active: formatRelative(session.lastSeenAt)
			})}
			onclick={() => onrevoke(session)}
		>
			{t('security-sessions-sign-out')}
		</Button>
	{/if}
</li>
