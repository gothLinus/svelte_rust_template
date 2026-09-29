<script lang="ts">
	import { oauthUrl } from '$lib/api';
	import ProviderIcon from '$lib/components/provider-icon.svelte';
	import { buttonVariants } from '$lib/components/ui/button';
	import type { OAuthProvider } from '$lib/types/api';
	import { cn } from '$lib/utils';

	/**
	 * "Continue with ..." buttons. They are plain links: the browser leaves the app, signs
	 * in at the provider and comes back through the API, which creates the account on the
	 * first visit.
	 */
	let {
		providers,
		redirectTo = null
	}: { providers: readonly OAuthProvider[]; redirectTo?: string | null } = $props();
</script>

{#if providers.length > 0}
	<div class={['grid gap-2', providers.length > 1 && 'sm:grid-cols-2']}>
		{#each providers as provider (provider.id)}
			<!-- An API endpoint, not a route of this app. -->
			<!-- eslint-disable svelte/no-navigation-without-resolve -->
			<a
				href={oauthUrl(provider.id, redirectTo)}
				data-sveltekit-reload
				class={cn(buttonVariants({ variant: 'outline' }), 'w-full')}
			>
				<ProviderIcon provider={provider.id} />
				{provider.name}
			</a>
			<!-- eslint-enable svelte/no-navigation-without-resolve -->
		{/each}
	</div>
{/if}
