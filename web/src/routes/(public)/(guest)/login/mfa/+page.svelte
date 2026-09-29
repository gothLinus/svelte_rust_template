<script lang="ts">
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import { onMount } from 'svelte';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { type SignedIn, api } from '$lib/api';
	import { PasskeyCancelled, finishSignIn, usePasskey } from '$lib/auth';
	import CodeInput from '$lib/components/code-input.svelte';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import OrSeparator from '$lib/components/or-separator.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { MfaMethod } from '$lib/types/api';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';

	type Status = { kind: 'loading' } | { kind: 'ready'; methods: MfaMethod[] } | { kind: 'expired' };
	let status = $state<Status>({ kind: 'loading' });
	let mode = $state<'totp' | 'recovery'>('totp');
	let code = $state('');
	const redirectTo = $derived(page.url.searchParams.get('redirectTo'));
	const form = new FormState(['code']);

	const has = (method: MfaMethod) => status.kind === 'ready' && status.methods.includes(method);
	// The code form, for the authenticator app or a recovery code. The server only offers
	// recovery codes while some are left, so a passkey may be the only way.
	const codeForm = $derived(mode === 'totp' ? has(MfaMethod.TOTP) : has(MfaMethod.RECOVERY_CODE));

	onMount(async () => {
		try {
			const { methods } = await api.auth.mfaPending();
			status = { kind: 'ready', methods };
			if (!methods.includes(MfaMethod.TOTP) && methods.includes(MfaMethod.RECOVERY_CODE))
				mode = 'recovery';
		} catch {
			status = { kind: 'expired' };
		}
	});

	async function done(me: SignedIn | undefined) {
		if (me) await finishSignIn({ kind: 'signedIn', me }, redirectTo);
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const me = await form.submit(
			() => (mode === 'totp' ? api.auth.mfaTotp(code) : api.auth.mfaRecoveryCode(code)),
			{ validation: validate({ code }, { code: mode === 'totp' ? rules.code : rules.required }) }
		);
		await done(me);
	}

	async function withPasskey() {
		const me = await form.submit(async () => {
			const options = await api.auth.mfaPasskeyOptions();
			try {
				return await api.auth.mfaPasskey(await usePasskey(options));
			} catch (error) {
				if (error instanceof PasskeyCancelled) return undefined;
				throw error;
			}
		});
		await done(me);
	}
</script>

<svelte:head><title>{t('mfa-page-title', { site: site.name })}</title></svelte:head>

<Card.Root>
	<Card.Header>
		<ShieldCheckIcon class="mb-1 size-5 text-muted-foreground" />
		<Card.Title class="text-lg"><h1>{t('mfa-title')}</h1></Card.Title>
		<Card.Description>
			{#if status.kind !== 'ready'}
				{t('mfa-description-pending')}
			{:else if !codeForm}
				{t('mfa-description-passkey')}
			{:else if mode === 'totp'}
				{t('mfa-description-totp')}
			{:else}
				{t('mfa-description-recovery')}
			{/if}
		</Card.Description>
	</Card.Header>
	<Card.Content>
		{#if status.kind === 'loading'}
			<Spinner class="mx-auto" />
		{:else if status.kind === 'expired'}
			<p class="text-sm text-muted-foreground">
				{t('mfa-expired')}
				<a href={resolve('/login')} class="underline">{t('mfa-expired-link')}</a>
			</p>
		{:else}
			<div class="flex flex-col gap-4">
				{#if has(MfaMethod.PASSKEY)}
					<Button onclick={withPasskey} disabled={form.pending}>
						<KeyRoundIcon />
						{t('mfa-passkey')}
					</Button>
					{#if codeForm}<OrSeparator />{/if}
				{/if}
				{#if codeForm}
					<form onsubmit={submit} novalidate>
						<Field.Group>
							<FormError message={form.error} />
							<FormField
								id="code"
								label={mode === 'totp' ? t('mfa-label-totp') : t('mfa-label-recovery')}
								error={form.fieldErrors.code}
							>
								{#if mode === 'totp'}
									<CodeInput
										id="code"
										required
										bind:value={code}
										oninput={() => form.clearField('code')}
										{...invalid('code', form.fieldErrors.code)}
									/>
								{:else}
									<Input
										id="code"
										autocomplete="off"
										autocapitalize="off"
										spellcheck={false}
										placeholder={t('mfa-recovery-placeholder')}
										class="font-mono"
										required
										bind:value={code}
										oninput={() => form.clearField('code')}
										{...invalid('code', form.fieldErrors.code)}
									/>
								{/if}
							</FormField>
							<Button
								type="submit"
								variant={has(MfaMethod.PASSKEY) ? 'outline' : 'default'}
								disabled={form.pending}
							>
								{#if form.pending}<Spinner />{/if}
								{t('mfa-submit')}
							</Button>
						</Field.Group>
					</form>
				{:else}
					<FormError message={form.error} />
				{/if}
				{#if has(MfaMethod.RECOVERY_CODE)}
					<Button
						variant="link"
						size="sm"
						class="mx-auto"
						onclick={() => {
							mode = mode === 'totp' ? 'recovery' : 'totp';
							code = '';
							form.reset();
						}}
					>
						{mode === 'recovery' && has(MfaMethod.TOTP)
							? t('mfa-use-totp')
							: mode === 'recovery'
								? t('mfa-back')
								: t('mfa-use-recovery')}
					</Button>
				{/if}
			</div>
		{/if}
	</Card.Content>
</Card.Root>
