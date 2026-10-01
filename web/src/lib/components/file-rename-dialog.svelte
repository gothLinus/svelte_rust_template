<script lang="ts">
	import FileRenameForm from '$lib/components/file-rename-form.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import type { StoredFile } from '$lib/types/api';

	let {
		open = $bindable(false),
		file = null,
		onsaved
	}: {
		open?: boolean;
		file?: StoredFile | null;
		onsaved: (file: StoredFile) => void;
	} = $props();
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-md">
		{#if file}
			<FileRenameForm
				{file}
				oncancel={() => (open = false)}
				onsaved={(saved) => {
					open = false;
					onsaved(saved);
				}}
			/>
		{/if}
	</Dialog.Content>
</Dialog.Root>
