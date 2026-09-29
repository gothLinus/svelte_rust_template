<script lang="ts">
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';
	import MailIcon from '@lucide/svelte/icons/mail';
	import SmartphoneIcon from '@lucide/svelte/icons/smartphone';
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import {
		PasskeyCancelled,
		finishSignIn,
		oauthErrorMessage,
		passkeysSupported,
		usePasskey
	} from '$lib/auth';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import OrSeparator from '$lib/components/or-separator.svelte';
	import PasswordInput from '$lib/components/password-input.svelte';
	import SocialSignIn from '$lib/components/social-sign-in.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';

	let { data } = $props();

	let identifier = $state('');
	let password = $state('');
	const form = new FormState(['identifier', 'password']);
	const redirectTo = $derived(page.url.searchParams.get('redirectTo'));
	const withRedirect = (path: string) =>
		redirectTo ? `${path}?redirectTo=${encodeURIComponent(redirectTo)}` : path;

	form.error = oauthErrorMessage(page.url.searchParams.get('error'));

	let canUsePasskeys = $state(false);
	onMount(() => (canUsePasskeys = passkeysSupported()));

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const result = await form.submit(() => api.auth.login({ identifier, password }), {
			validation: validate(
				{ identifier, password },
				{ identifier: rules.required, password: rules.required }
			)
		});
		if (!result) return;
		password = '';
		await finishSignIn(result, redirectTo);
	}

	async function signInWithPasskey() {
		const me = await form.submit(async () => {
			const options = await api.auth.passkeyOptions();
			try {
				return await api.auth.passkeyLogin(await usePasskey(options));
			} catch (error) {
				if (error instanceof PasskeyCancelled) return undefined;
				throw error;
			}
		});
		if (me) await finishSignIn({ kind: 'signedIn', me }, redirectTo);
	}
</script>

<svelte:head><title>{t('login-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		<Card.Title class="text-lg"><h1>{t('login-title')}</h1></Card.Title>
		<Card.Description>{t('login-description')}</Card.Description>
	</Card.Header>
	<Card.Content>
		<div class="flex flex-col gap-4">
			<FormError message={form.error} />
			<SocialSignIn providers={data.methods.providers} {redirectTo} />
			{#if data.methods.providers.length > 0}<OrSeparator />{/if}
			<form onsubmit={submit} novalidate>
				<Field.Group>
					<FormField
						id="identifier"
						label={t('login-identifier')}
						error={form.fieldErrors.identifier}
					>
						<Input
							id="identifier"
							autocomplete="username"
							autocapitalize="off"
							spellcheck={false}
							required
							bind:value={identifier}
							oninput={() => form.clearField('identifier')}
							{...invalid('identifier', form.fieldErrors.identifier)}
						/>
					</FormField>
					<Field.Field>
						<Field.Label for="password">{t('login-password')}</Field.Label>
						<PasswordInput
							id="password"
							autocomplete="current-password"
							required
							bind:value={password}
							oninput={() => form.clearField('password')}
							{...invalid('password', form.fieldErrors.password)}
						/>
						{#if form.fieldErrors.password}
							<Field.Error id="password-error">{form.fieldErrors.password}</Field.Error>
						{/if}
						<div class="text-right">
							<a
								href={resolve('/forgot-password')}
								class="text-xs text-muted-foreground underline-offset-4 hover:text-foreground hover:underline"
							>
								{t('login-forgot')}
							</a>
						</div>
					</Field.Field>
					<Button type="submit" disabled={form.pending}>
						{#if form.pending}<Spinner />{/if}
						{t('login-submit')}
					</Button>
				</Field.Group>
			</form>
			<OrSeparator />
			<div class="grid gap-2">
				{#if canUsePasskeys}
					<Button variant="outline" disabled={form.pending} onclick={signInWithPasskey}>
						<KeyRoundIcon />
						{t('login-by-passkey')}
					</Button>
				{/if}
				<!-- Paths with a query string; `resolve` does not take one. -->
				<!-- eslint-disable svelte/no-navigation-without-resolve -->
				<Button variant="outline" href={withRedirect('/login/email')}>
					<MailIcon />
					{t('login-by-email')}
				</Button>
				{#if data.methods.textChannels.length > 0}
					<Button variant="outline" href={withRedirect('/login/phone')}>
						<SmartphoneIcon />
						{t('login-by-text')}
					</Button>
				{/if}
				<!-- eslint-enable svelte/no-navigation-without-resolve -->
			</div>
			<p class="text-center text-sm text-muted-foreground">
				{t('login-no-account')}
				<!-- A same-site path with a query string. -->
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
				<a href={withRedirect('/register')} class="text-foreground underline underline-offset-4"
					>{t('login-create-account')}</a
				>
			</p>
		</div>
	</Card.Content>
</Card.Root>
