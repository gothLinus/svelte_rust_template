<script lang="ts">
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import CookieIcon from '@lucide/svelte/icons/cookie';
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';
	import MailIcon from '@lucide/svelte/icons/mail';
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import { resolve } from '$app/paths';
	import Logo from '$lib/components/logo.svelte';
	import ThemeToggle from '$lib/components/theme-toggle.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';

	let { data } = $props();

	const features = [
		{ key: 'sessions', icon: CookieIcon },
		{ key: 'passwords', icon: KeyRoundIcon },
		{ key: 'email', icon: MailIcon },
		{ key: 'roles', icon: ShieldCheckIcon }
	];
</script>

<svelte:head><title>{site.name}</title></svelte:head>

<div class="flex min-h-svh flex-col">
	<header class="mx-auto flex w-full max-w-5xl items-center justify-between gap-2 p-4 sm:p-6">
		<Logo />
		<nav class="flex items-center gap-1 sm:gap-2">
			<ThemeToggle />
			{#if data.me}
				<Button href={resolve('/dashboard')} size="sm">{t('landing-dashboard')}</Button>
			{:else}
				<Button href={resolve('/login')} variant="ghost" size="sm">{t('landing-sign-in')}</Button>
				<Button href={resolve('/register')} size="sm" class="hidden sm:inline-flex"
					>{t('landing-create-account')}</Button
				>
			{/if}
		</nav>
	</header>

	<main
		class="mx-auto flex w-full max-w-5xl flex-1 flex-col justify-center gap-10 px-4 pt-6 pb-[calc(--spacing(10)+env(safe-area-inset-bottom))] sm:gap-16 sm:px-6 sm:py-16"
	>
		<section class="flex max-w-2xl flex-col gap-6">
			<h1 class="text-3xl font-semibold tracking-tight text-balance sm:text-5xl">
				{t('landing-headline')}
			</h1>
			<p class="text-base text-pretty text-muted-foreground sm:text-lg">{site.description}</p>
			<div class="flex flex-col gap-3 sm:flex-row">
				{#if data.me}
					<Button href={resolve('/dashboard')} size="lg">
						{t('landing-go-to-dashboard')}
						<ArrowRightIcon data-icon="inline-end" />
					</Button>
				{:else}
					<Button href={resolve('/register')} size="lg">
						{t('landing-get-started')}
						<ArrowRightIcon data-icon="inline-end" />
					</Button>
					<Button href={resolve('/login')} variant="outline" size="lg"
						>{t('landing-sign-in')}</Button
					>
				{/if}
			</div>
		</section>

		<section class="grid gap-3 sm:grid-cols-2 sm:gap-4">
			{#each features as feature (feature.key)}
				<Card.Root size="sm">
					<Card.Header>
						<feature.icon class="mb-2 size-5 text-muted-foreground" />
						<Card.Title>{t(`landing-feature-${feature.key}-title`)}</Card.Title>
						<Card.Description>{t(`landing-feature-${feature.key}-description`)}</Card.Description>
					</Card.Header>
				</Card.Root>
			{/each}
		</section>
	</main>
</div>
