<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { api } from '$lib/api';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { t } from '$lib/i18n';

	/** The address, and changing it: the new one only applies once its link is opened. */
	let { email, verified }: { email: string; verified: boolean } = $props();

	let open = $state(false);
	let newEmail = $state('');
	const form = new FormState(['email']);

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const done = await form.submit(
			async () => {
				await api.me.changeEmail({ email: newEmail });
				return true;
			},
			{ validation: validate({ email: newEmail }, { email: rules.email }) }
		);
		if (!done) return;
		open = false;
		toast.success(t('profile-email-link-sent', { email: newEmail.trim() }));
	}
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('profile-email-title')}</Card.Title>
		<Card.Description>{t('profile-email-description')}</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-wrap items-center gap-2">
		<span class="text-sm font-medium break-all">{email}</span>
		{#if verified}
			<Badge variant="secondary">{t('profile-email-verified')}</Badge>
		{:else}
			<Badge variant="outline">{t('profile-email-unverified')}</Badge>
		{/if}
		<Button
			variant="outline"
			size="sm"
			class="ml-auto"
			aria-label={t('profile-email-change-label')}
			onclick={() => ((open = true), (newEmail = ''), form.reset())}
		>
			{t('profile-email-change')}
		</Button>
	</Card.Content>
</Card.Root>

<Dialog.Root bind:open>
	<Dialog.Content>
		<form onsubmit={submit} class="contents" novalidate>
			<Dialog.Header>
				<Dialog.Title>{t('profile-email-dialog-title')}</Dialog.Title>
				<Dialog.Description>
					{t('profile-email-dialog-description')}
				</Dialog.Description>
			</Dialog.Header>
			<Field.Group>
				<FormError message={form.error} />
				<FormField id="new-email" label={t('profile-email-new')} error={form.fieldErrors.email}>
					<Input
						id="new-email"
						type="email"
						autocomplete="email"
						required
						bind:value={newEmail}
						oninput={() => form.clearField('email')}
						{...invalid('new-email', form.fieldErrors.email)}
					/>
				</FormField>
			</Field.Group>
			<Dialog.Footer>
				<Button type="submit" disabled={form.pending}>
					{#if form.pending}<Spinner />{/if}
					{t('profile-email-send')}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
