<script lang="ts">
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import * as Alert from '$lib/components/ui/alert';

	let { message }: { message: string | null } = $props();
</script>

{#if message}
	<Alert.Root variant="destructive">
		<CircleAlertIcon />
		<Alert.Description>{message}</Alert.Description>
	</Alert.Root>
{/if}
