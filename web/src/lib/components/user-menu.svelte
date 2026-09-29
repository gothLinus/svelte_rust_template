<script lang="ts">
	import AccountMenuItems from '$lib/components/account-menu-items.svelte';
	import UserAvatar from '$lib/components/user-avatar.svelte';
	import { buttonVariants } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import type { User } from '$lib/types/api';
	import { t } from '$lib/i18n';
	import { cn } from '$lib/utils';

	let { user, class: className }: { user: User; class?: string } = $props();
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class={cn(buttonVariants({ variant: 'ghost', size: 'icon' }), 'rounded-xl', className)}
		aria-label={t('nav-account-menu')}
	>
		<UserAvatar name={user.username} />
	</DropdownMenu.Trigger>
	<DropdownMenu.Content class="w-72 max-w-[calc(100vw-2rem)]" align="end" sideOffset={4}>
		<AccountMenuItems {user} />
	</DropdownMenu.Content>
</DropdownMenu.Root>
