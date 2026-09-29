<script lang="ts">
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import MailCheckIcon from '@lucide/svelte/icons/mail-check';
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, errorMessage } from '$lib/api';
	import { finishSignIn, lifetimes } from '$lib/auth';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Spinner } from '$lib/components/ui/spinner';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';
	import { takeToken } from '$lib/helpers/token';

	// The link is only used when the user clicks: mail scanners that open links (and run
	// scripts) must not spend it, and a forwarded link must not sign anyone in unasked.
	type Status =
		| { kind: 'loading' }
		| { kind: 'ready'; token: string }
		| { kind: 'signingIn' }
		| { kind: 'failed'; message: string };
	let status = $state<Status>({ kind: 'loading' });

	onMount(() => {
		const token = takeToken(page.url.pathname);
		status = token ? { kind: 'ready', token } : { kind: 'failed', message: t('magic-incomplete') };
	});

	async function signIn() {
		if (status.kind !== 'ready') return;
		const { token } = status;
		status = { kind: 'signingIn' };
		try {
			await finishSignIn(await api.auth.magicLink({ token }), null);
		} catch (error) {
			status = { kind: 'failed', message: errorMessage(error) };
		}
	}
</script>

<svelte:head><title>{t('magic-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		{#if status.kind === 'failed'}
			<CircleXIcon class="mb-1 size-5 text-destructive" />
			<Card.Title class="text-lg"><h1>{t('magic-failed-title')}</h1></Card.Title>
			<Card.Description>
				{t('magic-failed-description', {
					reason: status.message,
					minutes: lifetimes(page.data.methods).signInMinutes
				})}
			</Card.Description>
		{:else}
			<MailCheckIcon class="mb-1 size-5 text-muted-foreground" />
			<Card.Title class="text-lg"><h1>{t('magic-title', { site: site.name })}</h1></Card.Title>
			<Card.Description>{t('magic-description')}</Card.Description>
		{/if}
	</Card.Header>
	<Card.Footer>
		{#if status.kind === 'failed'}
			<Button href={resolve('/login/email')} class="w-full">{t('magic-retry')}</Button>
		{:else}
			<Button class="w-full" disabled={status.kind !== 'ready'} onclick={signIn}>
				{#if status.kind === 'signingIn'}<Spinner />{/if}
				{t('magic-continue')}
			</Button>
		{/if}
	</Card.Footer>
</Card.Root>
