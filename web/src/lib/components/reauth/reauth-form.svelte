<script lang="ts">
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';
	import { onMount } from 'svelte';
	import { api, errorMessage } from '$lib/api';
	import { PasskeyCancelled, passkeysSupported, usePasskey } from '$lib/auth';
	import CodeInput from '$lib/components/code-input.svelte';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import PasswordInput from '$lib/components/password-input.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Field from '$lib/components/ui/field';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { ReauthMethod } from '$lib/types/api';
	import { t } from '$lib/i18n';

	let { ondone }: { ondone: () => void } = $props();

	type Status =
		| { kind: 'loading' }
		| { kind: 'ready'; methods: ReauthMethod[] }
		| { kind: 'failed'; message: string };
	let status = $state<Status>({ kind: 'loading' });
	let method = $state<ReauthMethod>(ReauthMethod.PASSWORD);
	let password = $state('');
	let code = $state('');
	let emailSent = $state(false);
	// The request sends either as `secret`; a wrong one comes back as a `password` or `code`
	// field error, a malformed request as `secret` or `method`. All belong to the one input.
	const SECRET_FIELDS = ['password', 'code', 'secret', 'method'] as const;
	const form = new FormState(SECRET_FIELDS);
	const secretError = $derived(SECRET_FIELDS.map((field) => form.fieldErrors[field]).find(Boolean));

	const others = $derived(
		status.kind === 'ready' ? status.methods.filter((other) => other !== method) : []
	);
	const KNOWN = [
		ReauthMethod.PASSKEY,
		ReauthMethod.PASSWORD,
		ReauthMethod.TOTP,
		ReauthMethod.EMAIL_CODE
	];
	function switchLabel(other: ReauthMethod): string {
		switch (other) {
			case ReauthMethod.PASSKEY:
				return t('reauth-passkey-instead');
			case ReauthMethod.PASSWORD:
				return t('reauth-password-instead');
			case ReauthMethod.TOTP:
				return t('reauth-totp-instead');
			default:
				return t('reauth-email-instead');
		}
	}

	onMount(async () => {
		try {
			const methods = (await api.me.reauthMethods()).methods.filter(
				(m) => KNOWN.includes(m) && (m !== ReauthMethod.PASSKEY || passkeysSupported())
			);
			status = { kind: 'ready', methods };
			method = methods[0] ?? ReauthMethod.PASSWORD;
		} catch (error) {
			status = { kind: 'failed', message: errorMessage(error) };
		}
	});

	function choose(next: ReauthMethod) {
		method = next;
		code = '';
		form.reset();
	}

	function clearSecret() {
		for (const field of SECRET_FIELDS) form.clearField(field);
	}

	async function withPasskey() {
		const done = await form.submit(async () => {
			const options = await api.me.reauthPasskeyOptions();
			try {
				await api.me.reauthPasskey(await usePasskey(options));
			} catch (error) {
				if (error instanceof PasskeyCancelled) return false;
				throw error;
			}
			return true;
		});
		if (done) ondone();
	}

	async function sendEmailCode() {
		const sent = await form.submit(async () => {
			await api.me.reauthEmailCode();
			return true;
		});
		if (sent) emailSent = true;
	}

	async function submit(event: SubmitEvent) {
		event.preventDefault();
		const current = method;
		const done = await form.submit(
			async () => {
				const secret = current === ReauthMethod.PASSWORD ? password : code;
				await api.me.reauthenticate({ method: current, secret });
				return true;
			},
			{
				validation:
					current === ReauthMethod.PASSWORD
						? validate({ password }, { password: rules.required })
						: validate({ code }, { code: rules.code })
			}
		);
		if (done) ondone();
	}
</script>

<Dialog.Header>
	<Dialog.Title>{t('reauth-title')}</Dialog.Title>
	<Dialog.Description>{t('reauth-description')}</Dialog.Description>
</Dialog.Header>

{#if status.kind === 'loading'}
	<Spinner class="mx-auto" />
{:else if status.kind === 'failed'}
	<FormError message={status.message} />
{:else}
	<div class="flex flex-col gap-4">
		{#if method === ReauthMethod.PASSKEY}
			<FormError message={form.error} />
			<Button onclick={withPasskey} disabled={form.pending}>
				{#if form.pending}<Spinner />{:else}<KeyRoundIcon />{/if}
				{t('reauth-use-passkey')}
			</Button>
		{:else if method === ReauthMethod.EMAIL_CODE && !emailSent}
			<FormError message={form.error} />
			<Button onclick={sendEmailCode} disabled={form.pending}>
				{#if form.pending}<Spinner />{/if}
				{t('reauth-email-me')}
			</Button>
		{:else}
			<form onsubmit={submit} novalidate>
				<Field.Group>
					<FormError message={form.error} />
					{#if method === ReauthMethod.PASSWORD}
						<FormField id="reauth-password" label={t('reauth-password-label')} error={secretError}>
							<PasswordInput
								id="reauth-password"
								autocomplete="current-password"
								required
								bind:value={password}
								oninput={clearSecret}
								{...invalid('reauth-password', secretError)}
							/>
						</FormField>
					{:else}
						<FormField
							id="reauth-code"
							label={method === ReauthMethod.TOTP ? t('reauth-code-app') : t('reauth-code-email')}
							error={secretError}
						>
							<CodeInput
								id="reauth-code"
								required
								bind:value={code}
								oninput={clearSecret}
								{...invalid('reauth-code', secretError)}
							/>
						</FormField>
					{/if}
					<Button type="submit" disabled={form.pending}>
						{#if form.pending}<Spinner />{/if}
						{t('reauth-confirm')}
					</Button>
				</Field.Group>
			</form>
		{/if}
		{#each others as other (other)}
			<Button variant="link" size="sm" class="mx-auto" onclick={() => choose(other)}>
				{switchLabel(other)}
			</Button>
		{/each}
	</div>
{/if}
