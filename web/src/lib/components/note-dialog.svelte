<script lang="ts">
	import NoteForm from '$lib/components/note-form.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import type { Note } from '$lib/types/api';

	let {
		open = $bindable(false),
		note = null,
		onsaved
	}: {
		open?: boolean;
		note?: Note | null;
		onsaved: (note: Note, created: boolean) => void;
	} = $props();
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-lg">
		<NoteForm
			{note}
			oncancel={() => (open = false)}
			onsaved={(saved, created) => {
				open = false;
				onsaved(saved, created);
			}}
		/>
	</Dialog.Content>
</Dialog.Root>
