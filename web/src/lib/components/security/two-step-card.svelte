<script lang="ts">
	import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
	import SmartphoneIcon from '@lucide/svelte/icons/smartphone';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import KeyIcon from '@lucide/svelte/icons/key';
	import { toast } from 'svelte-sonner';
	import { tabSync } from '$lib/auth';
	import { api, errorMessage } from '$lib/api';
	import CodeInput from '$lib/components/code-input.svelte';
	import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import QrCode from '$lib/components/qr-code.svelte';
	import RecoveryCodesDialog from '$lib/components/security/recovery-codes-dialog.svelte';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Field from '$lib/components/ui/field';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import type { SecurityOverview, TotpSetup } from '$lib/types/api';
	import { RECOVERY_CODE_COUNT } from '$lib/types/generated/limits';
	import { SECURITY } from '$lib/helpers/dependencies';
	import { t } from '$lib/i18n';

	let { security }: { security: SecurityOverview } = $props();

	let recoveryCodes = $state<string[] | null>(null);

	let setup = $state<TotpSetup | null>(null);
	let setupCode = $state('');
	const setupForm = new FormState(['code']);
	let starting = $state(false);

	async function startSetup() {
		starting = true;
		try {
			setup = await api.me.startTotp();
			setupCode = '';
			setupForm.reset();
		} catch (error) {
			toast.error(errorMessage(error));
		} finally {
			starting = false;
		}
	}

	async function confirmSetup(event: SubmitEvent) {
		event.preventDefault();
		const added = await setupForm.submit(() => api.me.confirmTotp(setupCode), {
			validation: validate({ code: setupCode }, { code: rules.code })
		});
		if (!added) return;
		setup = null;
		toast.success(t('security-totp-added'));
		if (added.recoveryCodes) recoveryCodes = added.recoveryCodes.codes;
		await tabSync.refresh(SECURITY);
	}

	let removeOpen = $state(false);
	let removeCode = $state('');
	const removeForm = new FormState(['code']);

	async function remove(event: SubmitEvent) {
		event.preventDefault();
		const done = await removeForm.submit(
			async () => {
				await api.me.removeTotp(removeCode);
				return true;
			},
			{ validation: validate({ code: removeCode }, { code: rules.code }) }
		);
		if (!done) return;
		removeOpen = false;
		toast.success(t('security-totp-removed'));
		await tabSync.refresh(SECURITY);
	}

	let regenerateOpen = $state(false);

	async function regenerate() {
		try {
			recoveryCodes = (await api.me.regenerateRecoveryCodes()).codes;
			await tabSync.refresh(SECURITY);
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}

	async function copySecret(secret: string) {
		try {
			await navigator.clipboard.writeText(secret);
			toast.success(t('security-totp-key-copied'));
		} catch {
			toast.error(t('security-totp-key-copy-failed'));
		}
	}

	const grouped = (secret: string) => secret.match(/.{1,4}/g)?.join(' ') ?? secret;
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2} class="flex items-center gap-2">
			{t('security-two-step-title')}
			{#if security.mfaEnabled}
				<Badge variant="secondary"><ShieldCheckIcon />{t('security-two-step-on')}</Badge>
			{:else}
				<Badge variant="outline">{t('security-two-step-off')}</Badge>
			{/if}
		</Card.Title>
		<Card.Description>
			{t('security-two-step-description')}
		</Card.Description>
	</Card.Header>
	<Card.Content>
		<ul class="divide-y">
			<li class="flex items-center gap-3 py-2.5 first:pt-0">
				<div class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-muted">
					<SmartphoneIcon class="size-4 text-muted-foreground" />
				</div>
				<div class="flex min-w-0 flex-1 flex-col">
					<span class="text-sm font-medium">{t('security-totp-title')}</span>
					<span class="text-xs text-muted-foreground">
						{security.totpEnabled ? t('security-totp-enabled') : t('security-totp-hint')}
					</span>
				</div>
				{#if security.totpEnabled}
					<Button
						variant="ghost"
						size="sm"
						aria-label={t('security-totp-remove-label')}
						onclick={() => ((removeOpen = true), (removeCode = ''), removeForm.reset())}
					>
						{t('security-totp-remove')}
					</Button>
				{:else}
					<Button
						variant="outline"
						size="sm"
						disabled={starting}
						aria-label={t('security-totp-setup-label')}
						onclick={startSetup}
					>
						{#if starting}<Spinner />{/if}
						{t('security-totp-setup')}
					</Button>
				{/if}
			</li>
			{#if security.mfaEnabled}
				<li class="flex items-center gap-3 py-2.5 last:pb-0">
					<div class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-muted">
						<KeyIcon class="size-4 text-muted-foreground" />
					</div>
					<div class="flex min-w-0 flex-1 flex-col">
						<span class="text-sm font-medium">{t('security-recovery-title')}</span>
						<span class="text-xs text-muted-foreground">
							{t('security-recovery-remaining', {
								remaining: security.recoveryCodesRemaining,
								total: RECOVERY_CODE_COUNT
							})}
						</span>
					</div>
					<Button
						variant="ghost"
						size="sm"
						aria-label={t('security-recovery-new-label')}
						onclick={() => (regenerateOpen = true)}
					>
						{t('security-recovery-new')}
					</Button>
				</li>
			{/if}
		</ul>
	</Card.Content>
</Card.Root>

<Dialog.Root
	open={setup !== null}
	onOpenChange={(open) => {
		if (!open) setup = null;
	}}
>
	<Dialog.Content>
		{#if setup}
			<form onsubmit={confirmSetup} class="contents" novalidate>
				<Dialog.Header>
					<Dialog.Title>{t('security-totp-setup-title')}</Dialog.Title>
					<Dialog.Description>
						{t('security-totp-setup-description')}
					</Dialog.Description>
				</Dialog.Header>
				<div class="flex flex-col items-center gap-3">
					<QrCode value={setup.uri} label={t('security-totp-qr-label')} class="size-44" />
					<div class="flex flex-col items-center gap-1">
						<span id="totp-secret-label" class="text-xs text-muted-foreground">
							{t('security-totp-key')}
						</span>
						<div class="flex items-center gap-1">
							<code
								aria-labelledby="totp-secret-label"
								class="rounded-xl bg-muted px-3 py-1.5 text-center text-xs break-all select-all"
							>
								{grouped(setup.secret)}
							</code>
							<Button
								variant="ghost"
								size="icon-sm"
								aria-label={t('security-totp-copy-key')}
								onclick={() => setup && copySecret(setup.secret)}
							>
								<CopyIcon />
							</Button>
						</div>
					</div>
				</div>
				<Field.Group>
					<FormError message={setupForm.error} />
					<FormField
						id="totp-code"
						label={t('security-totp-code')}
						error={setupForm.fieldErrors.code}
					>
						<CodeInput
							id="totp-code"
							required
							bind:value={setupCode}
							oninput={() => setupForm.clearField('code')}
							{...invalid('totp-code', setupForm.fieldErrors.code)}
						/>
					</FormField>
				</Field.Group>
				<Dialog.Footer>
					<Button type="submit" disabled={setupForm.pending}>
						{#if setupForm.pending}<Spinner />{/if}
						{t('security-totp-turn-on')}
					</Button>
				</Dialog.Footer>
			</form>
		{/if}
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={removeOpen}>
	<Dialog.Content>
		<form onsubmit={remove} class="contents" novalidate>
			<Dialog.Header>
				<Dialog.Title>{t('security-totp-remove-title')}</Dialog.Title>
				<Dialog.Description>{t('security-totp-remove-description')}</Dialog.Description>
			</Dialog.Header>
			<Field.Group>
				<FormError message={removeForm.error} />
				<FormField
					id="remove-code"
					label={t('security-totp-code')}
					error={removeForm.fieldErrors.code}
				>
					<CodeInput
						id="remove-code"
						required
						bind:value={removeCode}
						oninput={() => removeForm.clearField('code')}
						{...invalid('remove-code', removeForm.fieldErrors.code)}
					/>
				</FormField>
			</Field.Group>
			<Dialog.Footer>
				<Button type="submit" variant="destructive" disabled={removeForm.pending}>
					{#if removeForm.pending}<Spinner />{/if}
					{t('security-totp-remove')}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<ConfirmDialog
	bind:open={regenerateOpen}
	title={t('security-recovery-new-title')}
	description={t('security-recovery-new-description')}
	confirmLabel={t('security-recovery-new-confirm')}
	onconfirm={regenerate}
/>

<RecoveryCodesDialog bind:codes={recoveryCodes} />
