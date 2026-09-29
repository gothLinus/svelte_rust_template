<script lang="ts">
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { api } from '$lib/api';
	import { finishSignIn, lifetimes } from '$lib/auth';
	import CodeInput from '$lib/components/code-input.svelte';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import TextChannelPicker from '$lib/components/text-channel-picker.svelte';
	import RichText, { emphasize } from '$lib/components/rich-text.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { TextChannel } from '$lib/types/api';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';

	let { data } = $props();

	let phone = $state('');
	// svelte-ignore state_referenced_locally
	let channel = $state<TextChannel>(data.methods.textChannels[0] ?? TextChannel.SMS);
	let code = $state('');
	let sentTo = $state<string | null>(null);
	const redirectTo = $derived(page.url.searchParams.get('redirectTo'));
	const form = new FormState(['phone', 'code']);

	async function send(event: SubmitEvent) {
		event.preventDefault();
		const done = await form.submit(
			async () => {
				await api.auth.phoneCode({ phone, channel });
				return true;
			},
			{ validation: validate({ phone }, { phone: rules.phone }) }
		);
		if (done) sentTo = phone.trim();
	}

	async function verify(event: SubmitEvent) {
		event.preventDefault();
		if (!sentTo) return;
		const number = sentTo;
		const result = await form.submit(() => api.auth.verifyPhoneCode({ phone: number, code }), {
			validation: validate({ code }, { code: rules.code })
		});
		if (result) await finishSignIn(result, redirectTo);
	}
</script>

<svelte:head><title>{t('phone-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		<Card.Title class="text-lg"><h1>{t('phone-title')}</h1></Card.Title>
		<Card.Description>
			{#if sentTo}
				<RichText
					text={t('phone-sent', {
						phone: emphasize(sentTo),
						minutes: lifetimes(data.methods).textCodeMinutes
					})}
				/>
			{:else}
				{t('phone-description')}
			{/if}
		</Card.Description>
	</Card.Header>
	<Card.Content>
		{#if data.methods.textChannels.length === 0}
			<p class="text-sm text-muted-foreground">{t('phone-unavailable')}</p>
		{:else if sentTo}
			<form onsubmit={verify} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					<FormField id="code" label={t('phone-code')} error={form.fieldErrors.code}>
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
						{t('phone-submit')}
					</Button>
					<Button variant="ghost" size="sm" onclick={() => ((sentTo = null), (code = ''))}>
						{t('phone-another')}
					</Button>
				</Field.Group>
			</form>
		{:else}
			<form onsubmit={send} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					<FormField id="phone" label={t('phone-label')} error={form.fieldErrors.phone}>
						<Input
							id="phone"
							type="tel"
							autocomplete="tel"
							placeholder={t('phone-placeholder')}
							required
							bind:value={phone}
							oninput={() => form.clearField('phone')}
							{...invalid('phone', form.fieldErrors.phone)}
						/>
					</FormField>
					<TextChannelPicker channels={data.methods.textChannels} bind:value={channel} />
					<Button type="submit" disabled={form.pending}>
						{#if form.pending}<Spinner />{/if}
						{t('phone-send')}
					</Button>
				</Field.Group>
			</form>
		{/if}
	</Card.Content>
	<Card.Footer>
		<Button variant="link" size="sm" href={resolve('/login')} class="mx-auto">
			<ArrowLeftIcon />
			{t('phone-other-ways')}
		</Button>
	</Card.Footer>
</Card.Root>
