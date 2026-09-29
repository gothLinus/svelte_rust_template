<script lang="ts">
	import * as Avatar from '$lib/components/ui/avatar';
	import { initials } from '$lib/helpers/format';
	import { cn } from '$lib/utils';

	let { name, class: className }: { name: string; class?: string } = $props();
</script>

<Avatar.Root class={cn('size-8 rounded-xl', className)}>
	<Avatar.Fallback class="rounded-xl bg-primary/10 text-xs font-medium text-primary">
		{initials(name)}
	</Avatar.Fallback>
</Avatar.Root>
