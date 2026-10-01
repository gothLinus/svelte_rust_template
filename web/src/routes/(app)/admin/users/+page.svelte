<script lang="ts">
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, errorMessage } from '$lib/api';
	import { hasPermission, tabSync } from '$lib/auth';
	import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
	import CursorPagination from '$lib/components/cursor-pagination.svelte';
	import PageHeading from '$lib/components/page-heading.svelte';
	import RichText, { emphasize } from '$lib/components/rich-text.svelte';
	import UserAvatar from '$lib/components/user-avatar.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { buttonVariants } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { Input } from '$lib/components/ui/input';
	import { Permission, type User } from '$lib/types/api';
	import { MAX_USER_SEARCH_LENGTH, MIN_USER_SEARCH_LENGTH } from '$lib/types/generated/limits';
	import { ADMIN_USERS } from '$lib/helpers/dependencies';
	import { formatDate } from '$lib/helpers/format';
	import { t } from '$lib/i18n';
	import { CursorTrail } from '$lib/helpers/pagination';
	import { withQuery } from '$lib/helpers/url';
	import { cn } from '$lib/utils';

	let { data } = $props();

	const canManage = $derived(hasPermission(data.me, Permission.USERS_MANAGE));
	const canAudit = $derived(hasPermission(data.me, Permission.AUDIT_READ));
	// svelte-ignore state_referenced_locally
	let search = $state(data.search);
	let busy = $state(false);
	const trail = new CursorTrail();

	function navigate(changes: Record<string, string | null>) {
		// A same-site path built from the current URL.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		return goto(withQuery(page.url, changes), { keepFocus: true });
	}

	function submitSearch(event: SubmitEvent) {
		event.preventDefault();
		const term = search.trim();
		if (term.length > 0 && term.length < MIN_USER_SEARCH_LENGTH) return;
		trail.reset();
		void navigate({ search: term || null, after: null });
	}

	async function change(action: () => Promise<User>, success: (user: User) => string) {
		busy = true;
		try {
			const user = await action();
			toast.success(success(user));
			await tabSync.refresh(ADMIN_USERS);
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			busy = false;
		}
	}

	const toggleRole = (user: User, role: string) =>
		user.roles.includes(role)
			? change(
					() => api.admin.revokeRole(user.id, role),
					(u) => t('admin-role-revoked', { name: u.username, role })
				)
			: change(
					() => api.admin.grantRole(user.id, role),
					(u) => t('admin-role-granted', { name: u.username, role })
				);

	function viewActivity(user: User) {
		// The audit log filtered to the user: a resolved path with a query.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		return goto(`${resolve('/admin/audit')}?user=${encodeURIComponent(user.id)}`);
	}

	const enable = (user: User) =>
		change(
			() => api.admin.enable(user.id),
			(u) => t('admin-enabled', { name: u.username })
		);

	let disabling = $state<User | null>(null);
	let disableOpen = $state(false);

	const disable = (user: User) =>
		change(
			() => api.admin.disable(user.id),
			(u) => t('admin-disabled', { name: u.username })
		);
</script>

<PageHeading title={t('admin-users-title')} description={t('admin-users-description')} />

<form class="relative" role="search" onsubmit={submitSearch}>
	<SearchIcon
		class="pointer-events-none absolute top-1/2 left-3 size-4 -translate-y-1/2 text-muted-foreground"
	/>
	<Input
		type="search"
		class="pl-9"
		placeholder={t('admin-search-placeholder')}
		aria-label={t('admin-search-label')}
		minlength={MIN_USER_SEARCH_LENGTH}
		maxlength={MAX_USER_SEARCH_LENGTH}
		bind:value={search}
	/>
</form>

<Card.Root>
	<Card.Content>
		{#if data.users.items.length === 0}
			<p class="py-6 text-center text-sm text-muted-foreground">{t('admin-empty')}</p>
		{/if}
		<ul class="divide-y">
			{#each data.users.items as user (user.id)}
				{@const self = user.id === data.me.user.id}
				<li class="flex items-center gap-3 py-3 first:pt-0 last:pb-0 sm:gap-4">
					<UserAvatar name={user.username} />
					<div class="flex min-w-0 flex-1 flex-col">
						<span class="flex items-center gap-2 truncate text-sm font-medium">
							{user.username}
							{#if self}<span class="font-normal text-muted-foreground">{t('admin-you')}</span>{/if}
							{#if user.disabled}<Badge variant="destructive">{t('admin-disabled-badge')}</Badge
								>{/if}
						</span>
						<span class="truncate text-xs text-muted-foreground">{user.email}</span>
					</div>
					<div class="hidden gap-1 sm:flex">
						{#each user.roles as role (role)}
							<Badge variant={role === 'admin' ? 'default' : 'secondary'} class="capitalize">
								{role}
							</Badge>
						{/each}
					</div>
					<span class="hidden w-28 shrink-0 text-sm text-muted-foreground md:block">
						{formatDate(user.createdAt)}
					</span>
					{#if canManage || canAudit}
						<DropdownMenu.Root>
							<DropdownMenu.Trigger
								class={cn(buttonVariants({ variant: 'ghost', size: 'icon-sm' }))}
								aria-label={t('admin-manage-label', { name: user.username })}
								disabled={busy}
							>
								<EllipsisIcon />
							</DropdownMenu.Trigger>
							<DropdownMenu.Content align="end" class="w-56">
								{#if canAudit}
									<DropdownMenu.Item onSelect={() => viewActivity(user)}>
										{t('admin-view-activity')}
									</DropdownMenu.Item>
								{/if}
								{#if canAudit && canManage}<DropdownMenu.Separator />{/if}
								{#if canManage}
									<DropdownMenu.Label>{t('admin-menu-roles')}</DropdownMenu.Label>
									{#each data.roles as role (role.name)}
										{@const described = role.description
											? `role-${role.name}-description`
											: undefined}
										<DropdownMenu.CheckboxItem
											checked={user.roles.includes(role.name)}
											onCheckedChange={() => toggleRole(user, role.name)}
											aria-describedby={described}
										>
											<span class="flex min-w-0 flex-col">
												<span class="capitalize">{role.name}</span>
												{#if described}
													<!-- A description, not part of the item's name. -->
													<span
														id={described}
														aria-hidden="true"
														class="text-xs font-normal whitespace-normal text-muted-foreground"
													>
														{role.description}
													</span>
												{/if}
											</span>
										</DropdownMenu.CheckboxItem>
									{/each}
								{/if}
								{#if canManage && !self}
									<DropdownMenu.Separator />
									<DropdownMenu.Item
										variant={user.disabled ? 'default' : 'destructive'}
										onSelect={() =>
											user.disabled ? enable(user) : ((disabling = user), (disableOpen = true))}
									>
										{user.disabled ? t('admin-enable') : t('admin-disable')}
									</DropdownMenu.Item>
								{/if}
							</DropdownMenu.Content>
						</DropdownMenu.Root>
					{/if}
				</li>
			{/each}
		</ul>
	</Card.Content>
</Card.Root>

<CursorPagination
	label={t('admin-pages-label')}
	hasPrevious={trail.hasPrevious(data.after)}
	hasNext={data.users.nextCursor !== undefined}
	onPrevious={() => navigate({ after: trail.previous(data.after) })}
	onNext={() => {
		if (data.users.nextCursor)
			void navigate({ after: trail.next(data.after, data.users.nextCursor) });
	}}
/>

<ConfirmDialog
	bind:open={disableOpen}
	title={t('admin-disable-title', { name: disabling?.username ?? '' })}
	description={t('admin-disable-description')}
	confirmLabel={t('admin-disable-confirm')}
	onconfirm={async () => {
		if (disabling) await disable(disabling);
	}}
/>

{#if !canManage}
	<p class="text-sm text-muted-foreground">
		<RichText text={t('admin-view-only', { permission: emphasize('users:manage') })} tag="code" />
	</p>
{/if}
