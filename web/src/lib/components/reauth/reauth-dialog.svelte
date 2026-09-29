<script lang="ts">
	import { reauth } from '$lib/auth';
	import ReauthForm from '$lib/components/reauth/reauth-form.svelte';
	import * as Dialog from '$lib/components/ui/dialog';

	/**
	 * "Confirm it's you", for step-up re-authentication. Mounted once by the root layout;
	 * opens whenever a sensitive request needs it (see `reauth`).
	 */
</script>

<Dialog.Root
	open={reauth.open}
	onOpenChange={(open) => {
		if (!open) reauth.finish(false);
	}}
>
	<Dialog.Content>
		<ReauthForm ondone={() => reauth.finish(true)} />
	</Dialog.Content>
</Dialog.Root>
