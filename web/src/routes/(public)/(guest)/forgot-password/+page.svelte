<script lang="ts">
	import MailCheckIcon from '@lucide/svelte/icons/mail-check';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { lifetimes } from '$lib/auth';
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
	let sentTo = $state<string | null>(null);
	const form = new FormState(['email']);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const done = await form.submit(
			async () => {
				await api.auth.forgotPassword({ email });
				return true;
			},
			{ validation: validate({ email }, { email: rules.email }) }
		);
		if (done) sentTo = email.trim();
	}
</script>

<svelte:head><title>{t('forgot-page-title', { site: site.name })}</title></svelte:head>

{#if sentTo}
	<Card.Root>
		<Card.Header>
			<MailCheckIcon class="mb-2 size-6 text-muted-foreground" />
			<Card.Title class="text-lg"><h1>{t('forgot-sent-title')}</h1></Card.Title>
			<Card.Description>
				<RichText
					text={t('forgot-sent-description', {
						email: emphasize(sentTo),
						minutes: lifetimes(page.data.methods).passwordResetMinutes
					})}
				/>
			</Card.Description>
		</Card.Header>
		<Card.Footer>
			<Button href={resolve('/login')} variant="outline" class="w-full"
				>{t('forgot-sent-back')}</Button
			>
		</Card.Footer>
	</Card.Root>
{:else}
	<Card.Root>
		<Card.Header>
			<Card.Title class="text-lg"><h1>{t('forgot-title')}</h1></Card.Title>
			<Card.Description>{t('forgot-description')}</Card.Description>
		</Card.Header>
		<Card.Content>
			<form onsubmit={submit} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					<FormField id="email" label={t('forgot-email')} error={form.fieldErrors.email}>
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
					<Field.Field>
						<Button type="submit" disabled={form.pending}>
							{#if form.pending}<Spinner />{/if}
							{t('forgot-submit')}
						</Button>
						<Field.Description class="text-center">
							{t('forgot-remembered')} <a href={resolve('/login')}>{t('forgot-sign-in')}</a>
						</Field.Description>
					</Field.Field>
				</Field.Group>
			</form>
		</Card.Content>
	</Card.Root>
{/if}
