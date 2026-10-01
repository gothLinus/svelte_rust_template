<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import { goto } from '$app/navigation';
	import { page } from '$app/state';
	import CursorPagination from '$lib/components/cursor-pagination.svelte';
	import PageHeading from '$lib/components/page-heading.svelte';
	import UserAvatar from '$lib/components/user-avatar.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import type { AuditEvent } from '$lib/types/api';
	import { auditAction, auditDetail, isWarning } from '$lib/helpers/audit';
	import { describeUserAgent, formatDateTime, formatRelative } from '$lib/helpers/format';
	import { CursorTrail } from '$lib/helpers/pagination';
	import { withQuery } from '$lib/helpers/url';
	import { t } from '$lib/i18n';
	import { cn } from '$lib/utils';

	let { data } = $props();

	const trail = new CursorTrail();

	const filteredName = $derived(
		data.events.items.find((event) => event.user?.id === data.user)?.user?.username
	);

	function navigate(changes: Record<string, string | null>) {
		// A same-site path built from the current URL.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		return goto(withQuery(page.url, changes), { keepFocus: true });
	}

	function showAll() {
		trail.reset();
		void navigate({ user: null, after: null });
	}

	function filterBy(event: AuditEvent) {
		if (!event.user) return;
		trail.reset();
		void navigate({ user: event.user.id, after: null });
	}

	const causedBy = (event: AuditEvent) =>
		event.actor
			? t('admin-audit-by', { name: event.actor.username })
			: event.byOther
				? t('admin-audit-by-deleted')
				: undefined;
</script>

<PageHeading title={t('admin-audit-title')} description={t('admin-audit-description')} />

{#if data.user}
	<div class="flex items-center gap-2">
		<Badge variant="secondary">
			{filteredName
				? t('admin-audit-filtered', { name: filteredName })
				: t('admin-audit-filtered-unknown')}
		</Badge>
		<Button variant="ghost" size="sm" onclick={showAll}>
			<XIcon data-icon="inline-start" />
			{t('admin-audit-show-all')}
		</Button>
	</div>
{/if}

<Card.Root>
	<Card.Content>
		{#if data.events.items.length === 0}
			<p class="py-6 text-center text-sm text-muted-foreground">{t('admin-audit-empty')}</p>
		{/if}
		<ul class="divide-y">
			{#each data.events.items as event (event.id)}
				{@const detail = auditDetail(event, data.methods.providers)}
				{@const by = causedBy(event)}
				<li class="flex items-start gap-3 py-3 first:pt-0 last:pb-0 sm:gap-4">
					{#if event.user}
						<button
							type="button"
							class="shrink-0 rounded-full"
							aria-label={t('admin-audit-filtered', { name: event.user.username })}
							onclick={() => filterBy(event)}
						>
							<UserAvatar name={event.user.username} />
						</button>
					{/if}
					<div class="flex min-w-0 flex-1 flex-col gap-0.5">
						<span class="flex flex-wrap items-center gap-x-2 text-sm">
							<span class={cn('font-medium', isWarning(event) && 'text-destructive')}>
								{auditAction(event)}
							</span>
							{#if detail}<span class="text-muted-foreground">{detail}</span>{/if}
							{#if by}<span class="text-muted-foreground">{by}</span>{/if}
						</span>
						{#if event.user}
							<span class="truncate text-xs text-muted-foreground">
								<span class="sr-only">{t('admin-audit-account')}:</span>
								{event.user.username} · {event.user.email}
							</span>
						{/if}
						<span class="flex flex-col text-xs text-muted-foreground sm:flex-row sm:gap-1">
							<time title={formatDateTime(event.occurredAt)}>
								<span class="sr-only">{t('admin-audit-when')}:</span>
								{formatRelative(event.occurredAt)}
							</time>
							{#if event.ip || event.userAgent}
								<span>
									<span class="hidden sm:inline">·</span>
									<span class="sr-only">{t('admin-audit-from')}:</span>
									{[event.userAgent ? describeUserAgent(event.userAgent) : '', event.ip ?? '']
										.filter(Boolean)
										.join(' · ')}
								</span>
							{/if}
						</span>
					</div>
				</li>
			{/each}
		</ul>
	</Card.Content>
</Card.Root>

<CursorPagination
	label={t('admin-audit-pages-label')}
	hasPrevious={trail.hasPrevious(data.after)}
	hasNext={data.events.nextCursor !== undefined}
	onPrevious={() => navigate({ after: trail.previous(data.after) })}
	onNext={() => {
		if (data.events.nextCursor)
			void navigate({ after: trail.next(data.after, data.events.nextCursor) });
	}}
/>
