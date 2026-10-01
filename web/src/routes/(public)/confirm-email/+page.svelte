<script lang="ts">
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import CircleXIcon from '@lucide/svelte/icons/circle-x';
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api, errorMessage } from '$lib/api';
	import { lifetimes, SESSION, session, tabSync } from '$lib/auth';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { Spinner } from '$lib/components/ui/spinner';
	import { asSentence } from '$lib/helpers/format';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';
	import { takeToken } from '$lib/helpers/token';

	type Status =
		{ kind: 'confirming' } | { kind: 'confirmed' } | { kind: 'failed'; message: string };
	let status = $state<Status>({ kind: 'confirming' });

	onMount(async () => {
		const token = takeToken(page.url.pathname);
		if (!token) {
			status = { kind: 'failed', message: t('confirm-incomplete') };
			return;
		}
		try {
			await api.auth.confirmEmail({ token });
			status = { kind: 'confirmed' };
			await tabSync.refresh(SESSION);
		} catch (error) {
			status = { kind: 'failed', message: errorMessage(error) };
		}
	});
</script>

<svelte:head><title>{t('confirm-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		{#if status.kind === 'confirming'}
			<Spinner class="mb-1 size-5" />
			<Card.Title class="text-lg"><h1>{t('confirm-confirming')}</h1></Card.Title>
		{:else if status.kind === 'confirmed'}
			<CircleCheckIcon class="mb-1 size-5 text-muted-foreground" />
			<Card.Title class="text-lg"><h1>{t('confirm-done-title')}</h1></Card.Title>
			<Card.Description>{t('confirm-done-description')}</Card.Description>
		{:else}
			<CircleXIcon class="mb-1 size-5 text-destructive" />
			<Card.Title class="text-lg"><h1>{t('confirm-failed-title')}</h1></Card.Title>
			<Card.Description>
				{t('confirm-failed-description', {
					reason: asSentence(status.message),
					hours: lifetimes(page.data.methods).emailChangeHours
				})}
			</Card.Description>
		{/if}
	</Card.Header>
	{#if status.kind !== 'confirming'}
		<Card.Footer>
			{#if session.isAuthenticated}
				<Button href={resolve('/settings/profile')} class="w-full">{t('confirm-profile')}</Button>
			{:else}
				<Button href={resolve('/login')} class="w-full">{t('confirm-sign-in')}</Button>
			{/if}
		</Card.Footer>
	{/if}
</Card.Root>
