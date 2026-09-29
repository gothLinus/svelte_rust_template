<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { onMount } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { EVERYTHING, lifetimes, tabSync } from '$lib/auth';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import PasswordInput from '$lib/components/password-input.svelte';
	import * as Alert from '$lib/components/ui/alert';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';
	import { takeToken } from '$lib/helpers/token';

	let token = $state<string | null>(null);
	let checked = $state(false);
	let password = $state('');
	let confirmation = $state('');
	const form = new FormState(['password', 'confirmation']);

	onMount(() => {
		token = takeToken(page.url.pathname);
		checked = true;
	});

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		if (!token) return;
		const resetToken = token;
		const done = await form.submit(
			async () => {
				await api.auth.resetPassword({ token: resetToken, password });
				return true;
			},
			{
				validation: validate(
					{ password, confirmation },
					{ password: rules.newPassword, confirmation: rules.matches(() => password) }
				)
			}
		);
		if (!done) return;

		toast.success(t('reset-done'));
		tabSync.announce(EVERYTHING);
		await goto(resolve('/login'), { invalidateAll: true });
	}
</script>

<svelte:head><title>{t('reset-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		<Card.Title class="text-lg"><h1>{t('reset-title')}</h1></Card.Title>
		<Card.Description>{t('reset-description')}</Card.Description>
	</Card.Header>
	<Card.Content>
		{#if checked && !token}
			<Alert.Root variant="destructive">
				<Alert.Description>
					{t('reset-incomplete')}
					<a href={resolve('/forgot-password')}>{t('reset-request-new')}</a>
				</Alert.Description>
			</Alert.Root>
		{:else}
			<form onsubmit={submit} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					{#if form.error}
						<p class="text-sm text-muted-foreground">
							{t('reset-expiry', { minutes: lifetimes(page.data.methods).passwordResetMinutes })}
							<a href={resolve('/forgot-password')}>{t('reset-request-new')}</a>
						</p>
					{/if}
					<FormField id="password" label={t('reset-password')} error={form.fieldErrors.password}>
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
						label={t('reset-confirmation')}
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
					<Button type="submit" disabled={form.pending || !token}>
						{#if form.pending}<Spinner />{/if}
						{t('reset-submit')}
					</Button>
				</Field.Group>
			</form>
		{/if}
	</Card.Content>
</Card.Root>
