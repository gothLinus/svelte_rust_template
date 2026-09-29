<script lang="ts">
	import type { User } from '$lib/types/api';
	import UserAvatar from '$lib/components/user-avatar.svelte';

	let { user }: { user: User } = $props();
</script>

<UserAvatar name={user.username} />
<div class="grid flex-1 text-left text-sm leading-tight">
	<span class="truncate font-medium">{user.username}</span>
	<span class="truncate text-xs text-muted-foreground">{user.email}</span>
</div>
