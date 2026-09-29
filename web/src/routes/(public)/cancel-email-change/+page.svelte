<script lang="ts">
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import MailXIcon from '@lucide/svelte/icons/mail-x';
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, errorMessage } from '$lib/api';
	import { SESSION, tabSync } from '$lib/auth';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Spinner } from '$lib/components/ui/spinner';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';
	import { takeToken } from '$lib/helpers/token';

	// The link the old address gets when someone asks to move the account to another one.
	// Like sign-in links, it is only used on a click, never on load.
	type Status =
		| { kind: 'loading' }
		| { kind: 'ready'; token: string }
		| { kind: 'cancelling' }
		| { kind: 'cancelled' }
		| { kind: 'failed'; message: string };
	let status = $state<Status>({ kind: 'loading' });

	onMount(() => {
		const token = takeToken(page.url.pathname);
		status = token ? { kind: 'ready', token } : { kind: 'failed', message: t('cancel-incomplete') };
	});

	async function cancel() {
		if (status.kind !== 'ready') return;
		const { token } = status;
		status = { kind: 'cancelling' };
		try {
			await api.auth.cancelEmailChange({ token });
			status = { kind: 'cancelled' };
			await tabSync.refresh(SESSION);
		} catch (error) {
			status = { kind: 'failed', message: errorMessage(error) };
		}
	}
</script>

<svelte:head><title>{t('cancel-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		{#if status.kind === 'cancelled'}
			<CircleCheckIcon class="mb-1 size-5 text-muted-foreground" />
			<Card.Title class="text-lg"><h1>{t('cancel-done-title')}</h1></Card.Title>
			<Card.Description>
				{t('cancel-done-description')}
			</Card.Description>
		{:else if status.kind === 'failed'}
			<CircleXIcon class="mb-1 size-5 text-destructive" />
			<Card.Title class="text-lg"><h1>{t('cancel-failed-title')}</h1></Card.Title>
			<Card.Description>{status.message}</Card.Description>
		{:else}
			<MailXIcon class="mb-1 size-5 text-muted-foreground" />
			<Card.Title class="text-lg"><h1>{t('cancel-title')}</h1></Card.Title>
			<Card.Description>
				{t('cancel-description')}
			</Card.Description>
		{/if}
	</Card.Header>
	<Card.Footer>
		{#if status.kind === 'cancelled'}
			<Button href={resolve('/forgot-password')} class="w-full">{t('cancel-new-password')}</Button>
		{:else if status.kind === 'failed'}
			<Button href={resolve('/login')} variant="outline" class="w-full"
				>{t('cancel-sign-in')}</Button
			>
		{:else}
			<Button
				variant="destructive"
				class="w-full"
				disabled={status.kind !== 'ready'}
				onclick={cancel}
			>
				{#if status.kind === 'cancelling'}<Spinner />{/if}
				{t('cancel-submit')}
			</Button>
		{/if}
	</Card.Footer>
</Card.Root>
