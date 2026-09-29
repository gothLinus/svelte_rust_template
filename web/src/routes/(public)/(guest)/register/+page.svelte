<script lang="ts">
	import MailCheckIcon from '@lucide/svelte/icons/mail-check';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { finishSignIn } from '$lib/auth';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import OrSeparator from '$lib/components/or-separator.svelte';
	import PasswordInput from '$lib/components/password-input.svelte';
	import SocialSignIn from '$lib/components/social-sign-in.svelte';
	import RichText, { emphasize } from '$lib/components/rich-text.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { MAX_USERNAME_LENGTH } from '$lib/forms/validation';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';

	let { data } = $props();

	let email = $state('');
	let username = $state('');
	let password = $state('');
	let confirmation = $state('');
	let pendingEmail = $state<string | null>(null);

	const form = new FormState(['email', 'username', 'password', 'confirmation']);
	const redirectTo = $derived(page.url.searchParams.get('redirectTo'));
	const loginHref = $derived(
		redirectTo ? `/login?redirectTo=${encodeURIComponent(redirectTo)}` : '/login'
	);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const result = await form.submit(() => api.auth.register({ email, username, password }), {
			validation: validate(
				{ email, username, password, confirmation },
				{
					email: rules.email,
					username: rules.username,
					password: rules.newPassword,
					confirmation: rules.matches(() => password)
				}
			)
		});
		if (!result) return;

		password = confirmation = '';
		if (result.kind === 'verificationPending') {
			pendingEmail = result.email;
			return;
		}
		await finishSignIn(result, redirectTo);
	}
</script>

<svelte:head><title>{t('register-page-title', { site: site.name })}</title></svelte:head>

{#if pendingEmail}
	<Card.Root>
		<Card.Header>
			<MailCheckIcon class="mb-2 size-6 text-muted-foreground" />
			<Card.Title class="text-lg"><h1>{t('register-pending-title')}</h1></Card.Title>
			<Card.Description>
				<RichText text={t('register-pending-description', { email: emphasize(pendingEmail) })} />
			</Card.Description>
		</Card.Header>
		<Card.Footer>
			<Button href={resolve('/login')} variant="outline" class="w-full"
				>{t('register-pending-continue')}</Button
			>
		</Card.Footer>
	</Card.Root>
{:else}
	<Card.Root>
		<Card.Header>
			<Card.Title class="text-lg"><h1>{t('register-title')}</h1></Card.Title>
			<Card.Description>{t('register-description')}</Card.Description>
		</Card.Header>
		<Card.Content class="flex flex-col gap-4">
			{#if data.methods.providers.length > 0}
				<SocialSignIn providers={data.methods.providers} {redirectTo} />
				<OrSeparator />
			{/if}
			<form onsubmit={submit} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					<FormField
						id="username"
						label={t('register-username')}
						error={form.fieldErrors.username}
						description={t('register-username-hint')}
					>
						<Input
							id="username"
							autocomplete="username"
							autocapitalize="off"
							spellcheck={false}
							maxlength={MAX_USERNAME_LENGTH}
							required
							bind:value={username}
							oninput={() => form.clearField('username')}
							{...invalid('username', form.fieldErrors.username, { described: true })}
						/>
					</FormField>
					<FormField id="email" label={t('register-email')} error={form.fieldErrors.email}>
						<Input
							id="email"
							type="email"
							autocomplete="email"
							placeholder={t('common-email-placeholder')}
							required
							bind:value={email}
							oninput={() => form.clearField('email')}
							{...invalid('email', form.fieldErrors.email)}
						/>
					</FormField>
					<FormField id="password" label={t('register-password')} error={form.fieldErrors.password}>
						<PasswordInput
							id="password"
							autocomplete="new-password"
							required
							bind:value={password}
							oninput={() => form.clearField('password')}
							{...invalid('password', form.fieldErrors.password)}
						/>
					</FormField>
					<FormField
						id="confirmation"
						label={t('register-confirmation')}
						error={form.fieldErrors.confirmation}
					>
						<PasswordInput
							id="confirmation"
							autocomplete="new-password"
							required
							bind:value={confirmation}
							oninput={() => form.clearField('confirmation')}
							{...invalid('confirmation', form.fieldErrors.confirmation)}
						/>
					</FormField>
					<Field.Field>
						<Button type="submit" disabled={form.pending}>
							{#if form.pending}<Spinner />{/if}
							{t('register-submit')}
						</Button>
						<Field.Description class="text-center">
							<!-- `loginHref` is a same-site path with a query string. -->
							<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -->
							{t('register-have-account')} <a href={loginHref}>{t('register-sign-in')}</a>
						</Field.Description>
					</Field.Field>
				</Field.Group>
			</form>
		</Card.Content>
	</Card.Root>
{/if}
