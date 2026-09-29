<script lang="ts">
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { finishSignIn, lifetimes } from '$lib/auth';
	import CodeInput from '$lib/components/code-input.svelte';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import RichText, { emphasize } from '$lib/components/rich-text.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';

	let email = $state('');
	let code = $state('');
	let sentTo = $state<string | null>(null);
	const redirectTo = $derived(page.url.searchParams.get('redirectTo'));
	const form = new FormState(['email', 'code']);

	async function send(event: SubmitEvent) {
		event.preventDefault();
		const done = await form.submit(
			async () => {
				await api.auth.emailCode({ email });
				return true;
			},
			{ validation: validate({ email }, { email: rules.email }) }
		);
		if (done) sentTo = email.trim();
	}

	async function verify(event: SubmitEvent) {
		event.preventDefault();
		if (!sentTo) return;
		const address = sentTo;
		const result = await form.submit(() => api.auth.verifyEmailCode({ email: address, code }), {
			validation: validate({ code }, { code: rules.code })
		});
		if (result) await finishSignIn(result, redirectTo);
	}
</script>

<svelte:head><title>{t('login-email-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		<Card.Title class="text-lg"><h1>{t('login-email-title')}</h1></Card.Title>
		<Card.Description>
			{#if sentTo}
				<RichText
					text={t('login-email-sent', {
						email: emphasize(sentTo),
						minutes: lifetimes(page.data.methods).signInMinutes
					})}
				/>
			{:else}
				{t('login-email-description')}
			{/if}
		</Card.Description>
	</Card.Header>
	<Card.Content>
		{#if sentTo}
			<form onsubmit={verify} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					<FormField id="code" label={t('login-email-code')} error={form.fieldErrors.code}>
						<CodeInput
							id="code"
							required
							bind:value={code}
							oninput={() => form.clearField('code')}
							{...invalid('code', form.fieldErrors.code)}
						/>
					</FormField>
					<Button type="submit" disabled={form.pending}>
						{#if form.pending}<Spinner />{/if}
						{t('login-email-submit')}
					</Button>
					<Button variant="ghost" size="sm" onclick={() => ((sentTo = null), (code = ''))}>
						{t('login-email-another')}
					</Button>
				</Field.Group>
			</form>
		{:else}
			<form onsubmit={send} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					<FormField id="email" label={t('login-email-label')} error={form.fieldErrors.email}>
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
					<Button type="submit" disabled={form.pending}>
						{#if form.pending}<Spinner />{/if}
						{t('login-email-send')}
					</Button>
				</Field.Group>
			</form>
		{/if}
	</Card.Content>
	<Card.Footer>
		<Button variant="link" size="sm" href={resolve('/login')} class="mx-auto">
			<ArrowLeftIcon />
			{t('login-email-other-ways')}
		</Button>
	</Card.Footer>
</Card.Root>
