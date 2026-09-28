<script lang="ts">
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import MonitorSmartphoneIcon from '@lucide/svelte/icons/monitor-smartphone';
	import ShieldIcon from '@lucide/svelte/icons/shield';
	import { resolve } from '$app/paths';
	import PageHeading from '$lib/components/page-heading.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { formatDate, formatRelative } from '$lib/helpers/format';
	import { t } from '$lib/i18n';

	let { data } = $props();

	const user = $derived(data.me.user);
</script>

<PageHeading
	title={t('dashboard-title', { name: user.username })}
	description={t('dashboard-description')}
/>

<div class="grid grid-cols-2 gap-3 sm:grid-cols-3 sm:gap-4">
	<Card.Root size="sm">
		<Card.Header>
			<Card.Description class="flex items-center gap-2">
				<ShieldIcon class="size-4" />
				{t('dashboard-roles')}
			</Card.Description>
			<Card.Title class="flex flex-wrap gap-1">
				{#each user.roles as role (role)}
					<Badge variant={role === 'admin' ? 'default' : 'secondary'} class="capitalize">
						{role}
					</Badge>
				{:else}
					<span class="text-sm text-muted-foreground">{t('dashboard-roles-none')}</span>
				{/each}
			</Card.Title>
		</Card.Header>
	</Card.Root>
	<Card.Root size="sm">
		<Card.Header>
			<Card.Description class="flex items-center gap-2">
				<MonitorSmartphoneIcon class="size-4" />
				{t('dashboard-sessions')}
			</Card.Description>
			<Card.Title class="text-xl">{data.sessions.length}</Card.Title>
		</Card.Header>
	</Card.Root>
	<Card.Root size="sm" class="col-span-2 sm:col-span-1">
		<Card.Header>
			<Card.Description class="flex items-center gap-2">
				<CalendarIcon class="size-4" />
				{t('dashboard-member-since')}
			</Card.Description>
			<Card.Title class="text-xl">{formatDate(user.createdAt)}</Card.Title>
		</Card.Header>
	</Card.Root>
</div>

<Card.Root>
	<Card.Header>
		<Card.Title level={2}>{t('dashboard-recent-title')}</Card.Title>
		<Card.Description>{t('dashboard-recent-description')}</Card.Description>
	</Card.Header>
	<Card.Content>
		{#if data.recentNotes.length === 0}
			<p class="text-sm text-muted-foreground">{t('dashboard-recent-empty')}</p>
		{:else}
			<ul class="divide-y">
				{#each data.recentNotes as note (note.id)}
					<li class="flex items-center justify-between gap-4 py-2 first:pt-0 last:pb-0">
						<span class="truncate text-sm font-medium">{note.title}</span>
						<span class="shrink-0 text-xs text-muted-foreground">
							{formatRelative(note.updatedAt)}
						</span>
					</li>
				{/each}
			</ul>
		{/if}
	</Card.Content>
	<Card.Footer>
		<Button href={resolve('/notes')} variant="outline" size="sm" class="w-full sm:w-auto">
			{t('dashboard-all-notes')}
			<ArrowRightIcon data-icon="inline-end" />
		</Button>
	</Card.Footer>
</Card.Root>
