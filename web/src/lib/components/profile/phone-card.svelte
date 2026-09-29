<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { api, errorMessage } from '$lib/api';
	import { SESSION, tabSync } from '$lib/auth';
	import CodeInput from '$lib/components/code-input.svelte';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import TextChannelPicker from '$lib/components/text-channel-picker.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { TextChannel, type User } from '$lib/types/api';
	import { t } from '$lib/i18n';

	/** A phone number to sign in with (password or texted code). Saved once a code confirms it. */
	let { user, channels }: { user: User; channels: readonly TextChannel[] } = $props();

	let open = $state(false);
	let phone = $state('');
	// svelte-ignore state_referenced_locally
	let channel = $state<TextChannel>(channels[0] ?? TextChannel.SMS);
	let code = $state('');
	let sentTo = $state<string | null>(null);
	const form = new FormState(['phone', 'code']);

	function start() {
		open = true;
		phone = '';
		code = '';
		sentTo = null;
		form.reset();
	}

	async function send(event: SubmitEvent) {
		event.preventDefault();
		const done = await form.submit(
			async () => {
				await api.me.addPhone({ phone, channel });
				return true;
			},
			{ validation: validate({ phone }, { phone: rules.phone }) }
		);
		if (done) sentTo = phone.trim();
	}

	async function verify(event: SubmitEvent) {
		event.preventDefault();
		const me = await form.submit(() => api.me.verifyPhone(code), {
			validation: validate({ code }, { code: rules.code })
		});
		if (!me) return;
		open = false;
		await tabSync.refresh(SESSION);
		toast.success(t('profile-phone-saved'));
	}

	let removing = $state(false);

	async function remove() {
		removing = true;
		try {
			await api.me.removePhone();
			await tabSync.refresh(SESSION);
			toast.success(t('profile-phone-removed'));
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			removing = false;
		}
	}
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('profile-phone-title')}</Card.Title>
		<Card.Description>
			{t('profile-phone-description')}
		</Card.Description>
	</Card.Header>
	<Card.Content class="flex flex-wrap items-center gap-2">
		{#if user.phone}
			<span class="font-mono text-sm">{user.phone}</span>
			{#if user.phoneVerified}<Badge variant="secondary">{t('profile-phone-verified')}</Badge>{/if}
			<div class="ml-auto flex gap-1">
				<Button
					variant="ghost"
					size="sm"
					disabled={removing}
					aria-label={t('profile-phone-remove-label')}
					onclick={remove}
				>
					{t('profile-phone-remove')}
				</Button>
				{#if channels.length > 0}
					<Button
						variant="outline"
						size="sm"
						aria-label={t('profile-phone-change-label')}
						onclick={start}
					>
						{t('profile-phone-change')}
					</Button>
				{/if}
			</div>
		{:else if channels.length > 0}
			<span class="text-sm text-muted-foreground">{t('profile-phone-none')}</span>
			<Button
				variant="outline"
				size="sm"
				class="ml-auto"
				aria-label={t('profile-phone-add-label')}
				onclick={start}
			>
				{t('profile-phone-add')}
			</Button>
		{:else}
			<span class="text-sm text-muted-foreground">{t('profile-phone-unavailable')}</span>
		{/if}
	</Card.Content>
</Card.Root>

<Dialog.Root bind:open>
	<Dialog.Content>
		{#if sentTo}
			<form onsubmit={verify} class="contents" novalidate>
				<Dialog.Header>
					<Dialog.Title>{t('profile-phone-code-title')}</Dialog.Title>
					<Dialog.Description>
						{t('profile-phone-code-description', { phone: sentTo })}
					</Dialog.Description>
				</Dialog.Header>
				<Field.Group>
					<FormError message={form.error} />
					<FormField
						id="phone-code"
						label={t('profile-phone-code-label')}
						error={form.fieldErrors.code}
					>
						<CodeInput
							id="phone-code"
							required
							bind:value={code}
							oninput={() => form.clearField('code')}
							{...invalid('phone-code', form.fieldErrors.code)}
						/>
					</FormField>
				</Field.Group>
				<Dialog.Footer>
					<Button variant="ghost" onclick={() => (sentTo = null)}>
						{t('profile-phone-back')}
					</Button>
					<Button type="submit" disabled={form.pending}>
						{#if form.pending}<Spinner />{/if}
						{t('profile-phone-verify')}
					</Button>
				</Dialog.Footer>
			</form>
		{:else}
			<form onsubmit={send} class="contents" novalidate>
				<Dialog.Header>
					<Dialog.Title>
						{user.phone ? t('profile-phone-change-title') : t('profile-phone-add-title')}
					</Dialog.Title>
					<Dialog.Description>{t('profile-phone-dialog-description')}</Dialog.Description>
				</Dialog.Header>
				<Field.Group>
					<FormError message={form.error} />
					<FormField id="phone" label={t('profile-phone-label')} error={form.fieldErrors.phone}>
						<Input
							id="phone"
							type="tel"
							autocomplete="tel"
							placeholder={t('profile-phone-placeholder')}
							required
							bind:value={phone}
							oninput={() => form.clearField('phone')}
							{...invalid('phone', form.fieldErrors.phone)}
						/>
					</FormField>
					<TextChannelPicker {channels} bind:value={channel} />
				</Field.Group>
				<Dialog.Footer>
					<Button type="submit" disabled={form.pending}>
						{#if form.pending}<Spinner />{/if}
						{t('profile-phone-send')}
					</Button>
				</Dialog.Footer>
			</form>
		{/if}
	</Dialog.Content>
</Dialog.Root>
