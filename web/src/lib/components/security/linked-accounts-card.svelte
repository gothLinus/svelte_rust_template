<script lang="ts" module>
	const LINKING_KEY = 'linking-provider';

	/**
	 * The provider a "Link" click is on its way to. Linking leaves the app, so a
	 * `reauth_required` comes back as `?error=`; the security page then re-authenticates
	 * and starts the same link again.
	 */
	export function takeLinkingProvider(): string | null {
		try {
			const provider = sessionStorage.getItem(LINKING_KEY);
			sessionStorage.removeItem(LINKING_KEY);
			return provider;
		} catch {
			return null;
		}
	}

	function rememberLinking(provider: string): void {
		try {
			sessionStorage.setItem(LINKING_KEY, provider);
		} catch {
			// Storage is blocked: the user just clicks "Link" again after confirming.
		}
	}
</script>

<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { tabSync } from '$lib/auth';
	import { api, errorMessage, oauthLinkUrl } from '$lib/api';
	import ProviderIcon from '$lib/components/provider-icon.svelte';
	import { Button, buttonVariants } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import type { LinkedAccount } from '$lib/types/api';
	import type { OAuthProvider } from '$lib/types/api';
	import { SECURITY } from '$lib/helpers/dependencies';
	import { t } from '$lib/i18n';
	import { cn } from '$lib/utils';

	let {
		providers,
		linked
	}: { providers: readonly OAuthProvider[]; linked: readonly LinkedAccount[] } = $props();

	const rows = $derived([
		...providers.map((provider) => ({
			id: provider.id,
			name: provider.name,
			account: linked.find((account) => account.provider === provider.id)
		})),
		...linked
			.filter((account) => !providers.some((provider) => provider.id === account.provider))
			.map((account) => ({ id: account.provider, name: account.providerName, account }))
	]);

	let busy = $state<string | null>(null);

	async function unlink(id: string, name: string) {
		busy = id;
		try {
			await api.me.unlink(id);
			toast.success(t('security-linked-unlinked', { provider: name }));
			await tabSync.refresh(SECURITY);
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			busy = null;
		}
	}
</script>

{#if rows.length > 0}
	<Card.Root size="sm">
		<Card.Header>
			<Card.Title level={2}>{t('security-linked-title')}</Card.Title>
			<Card.Description>{t('security-linked-description')}</Card.Description>
		</Card.Header>
		<Card.Content>
			<ul class="divide-y">
				{#each rows as row (row.id)}
					<li class="flex items-center gap-3 py-2.5 first:pt-0 last:pb-0">
						<div class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-muted">
							<ProviderIcon provider={row.id} />
						</div>
						<div class="flex min-w-0 flex-1 flex-col">
							<span class="text-sm font-medium">{row.name}</span>
							<span class="truncate text-xs text-muted-foreground">
								{row.account
									? (row.account.email ?? t('security-linked-linked'))
									: t('security-linked-not-linked')}
							</span>
						</div>
						{#if row.account}
							<Button
								variant="ghost"
								size="sm"
								disabled={busy === row.id}
								aria-label={t('security-linked-unlink-label', { provider: row.name })}
								onclick={() => unlink(row.id, row.name)}
							>
								{t('security-linked-unlink')}
							</Button>
						{:else}
							<!-- An API endpoint, not a route of this app. -->
							<!-- eslint-disable svelte/no-navigation-without-resolve -->
							<a
								href={oauthLinkUrl(row.id)}
								data-sveltekit-reload
								aria-label={t('security-linked-link-label', { provider: row.name })}
								onclick={() => rememberLinking(row.id)}
								class={cn(buttonVariants({ variant: 'outline', size: 'sm' }))}
							>
								{t('security-linked-link')}
							</a>
							<!-- eslint-enable svelte/no-navigation-without-resolve -->
						{/if}
					</li>
				{/each}
			</ul>
		</Card.Content>
	</Card.Root>
{/if}
