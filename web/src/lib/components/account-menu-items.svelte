<script lang="ts">
	import LogOutIcon from '@lucide/svelte/icons/log-out';
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import UserRoundIcon from '@lucide/svelte/icons/user-round';
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api, errorMessage } from '$lib/api';
	import { EVERYTHING, tabSync } from '$lib/auth';
	import { t } from '$lib/i18n';
	import UserIdentity from '$lib/components/user-identity.svelte';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import type { User } from '$lib/types/api';

	let { user }: { user: User } = $props();

	async function signOut() {
		try {
			await api.auth.logout();
		} catch (error) {
			toast.error(errorMessage(error));
			return;
		}
		tabSync.announce(EVERYTHING);
		await goto(resolve('/login'), { invalidateAll: true });
	}
</script>

<DropdownMenu.Label class="p-0 font-normal">
	<div class="flex items-center gap-2 px-1 py-1.5"><UserIdentity {user} /></div>
</DropdownMenu.Label>
<DropdownMenu.Separator />
<DropdownMenu.Group>
	<DropdownMenu.Item onSelect={() => goto(resolve('/settings/profile'))}>
		<UserRoundIcon />
		{t('nav-profile')}
	</DropdownMenu.Item>
	<DropdownMenu.Item onSelect={() => goto(resolve('/settings/security'))}>
		<ShieldCheckIcon />
		{t('nav-security')}
	</DropdownMenu.Item>
</DropdownMenu.Group>
<DropdownMenu.Separator />
<DropdownMenu.Item onSelect={signOut}>
	<LogOutIcon />
	{t('nav-sign-out')}
</DropdownMenu.Item>
