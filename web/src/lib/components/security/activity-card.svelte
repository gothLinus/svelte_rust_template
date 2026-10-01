<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { api, errorMessage } from '$lib/api';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import type { AuditEvent, AuditEventPage } from '$lib/types/api';
	import { ACTIVITY_PAGE_SIZE, auditAction, auditDetail, isWarning } from '$lib/helpers/audit';
	import { describeUserAgent, formatDateTime, formatRelative } from '$lib/helpers/format';
	import { t } from '$lib/i18n';
	import { cn } from '$lib/utils';

	let {
		activity,
		providers = []
	}: { activity: AuditEventPage; providers?: readonly { id: string; name: string }[] } = $props();

	// Later pages are appended here; a reload of the page starts over from the first.
	let more = $state<AuditEvent[]>([]);
	let cursor = $state<string | undefined>(undefined);
	let loading = $state(false);

	$effect.pre(() => {
		more = [];
		cursor = activity.nextCursor;
	});

	const events = $derived([...activity.items, ...more]);

	async function loadMore() {
		if (!cursor) return;
		loading = true;
		try {
			const next = await api.me.activity({ after: cursor, limit: ACTIVITY_PAGE_SIZE });
			more = [...more, ...next.items];
			cursor = next.nextCursor;
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			loading = false;
		}
	}
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('security-activity-title')}</Card.Title>
		<Card.Description>{t('security-activity-description')}</Card.Description>
	</Card.Header>
	<Card.Content>
		{#if events.length === 0}
			<p class="text-sm text-muted-foreground">{t('security-activity-empty')}</p>
		{/if}
		<ul class="divide-y">
			{#each events as event (event.id)}
				{@const detail = auditDetail(event, providers)}
				<li class="flex flex-col py-2.5 first:pt-0 last:pb-0">
					<span class="flex flex-wrap items-center gap-x-2 gap-y-1 text-sm">
						<span class={cn('font-medium', isWarning(event) && 'text-destructive')}>
							{auditAction(event)}
						</span>
						{#if detail}<span class="text-muted-foreground">{detail}</span>{/if}
						{#if event.byOther}
							<Badge variant="secondary">{t('security-activity-by-admin')}</Badge>
						{/if}
					</span>
					<span class="flex flex-col text-xs text-muted-foreground sm:flex-row sm:gap-1">
						<time title={formatDateTime(event.occurredAt)}>{formatRelative(event.occurredAt)}</time>
						{#if event.userAgent}
							<span>
								<span class="hidden sm:inline">·</span>
								{describeUserAgent(event.userAgent)}
							</span>
						{/if}
						{#if event.ip}
							<span><span class="hidden sm:inline">·</span> {event.ip}</span>
						{/if}
					</span>
				</li>
			{/each}
		</ul>
	</Card.Content>
	{#if cursor}
		<Card.Footer class="justify-end">
			<Button variant="outline" size="sm" disabled={loading} onclick={loadMore}>
				{t('security-activity-more')}
			</Button>
		</Card.Footer>
	{/if}
</Card.Root>
