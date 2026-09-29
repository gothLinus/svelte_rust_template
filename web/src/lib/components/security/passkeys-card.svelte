<script lang="ts">
	import KeyRoundIcon from '@lucide/svelte/icons/key-round';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { api, errorMessage } from '$lib/api';
	import { PasskeyCancelled, createPasskey, passkeysSupported, tabSync } from '$lib/auth';
	import FormError from '$lib/components/form-error.svelte';
	import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import RecoveryCodesDialog from '$lib/components/security/recovery-codes-dialog.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { MAX_PASSKEY_NAME_LENGTH } from '$lib/forms/validation';
	import type { Passkey } from '$lib/types/api';
	import { SECURITY } from '$lib/helpers/dependencies';
	import { describeUserAgent, formatDate, formatRelative } from '$lib/helpers/format';
	import { t } from '$lib/i18n';

	let { passkeys, mfaEnabled }: { passkeys: Passkey[]; mfaEnabled: boolean } = $props();

	let supported = $state(true);
	onMount(() => (supported = passkeysSupported()));

	let recoveryCodes = $state<string[] | null>(null);
	let adding = $state(false);

	async function add() {
		adding = true;
		try {
			const options = await api.me.passkeyOptions();
			const request = await createPasskey(options, describeUserAgent(navigator.userAgent));
			const registered = await api.me.registerPasskey(request);
			toast.success(t('security-passkeys-added'));
			if (registered.recoveryCodes) recoveryCodes = registered.recoveryCodes.codes;
			await tabSync.refresh(SECURITY);
		} catch (error) {
			if (!(error instanceof PasskeyCancelled)) toast.error(errorMessage(error));
		} finally {
			adding = false;
		}
	}

	let renaming = $state<Passkey | null>(null);
	let name = $state('');
	const renameForm = new FormState(['name']);

	function startRename(passkey: Passkey) {
		renaming = passkey;
		name = passkey.name;
		renameForm.reset();
	}

	async function rename(event: SubmitEvent) {
		event.preventDefault();
		if (!renaming) return;
		const id = renaming.id;
		const done = await renameForm.submit(() => api.me.renamePasskey(id, name), {
			validation: validate({ name }, { name: rules.passkeyName })
		});
		if (!done) return;
		renaming = null;
		await tabSync.refresh(SECURITY);
	}

	let removing = $state<Passkey | null>(null);
	let removeOpen = $state(false);

	async function remove() {
		const passkey = removing;
		if (!passkey) return;
		try {
			await api.me.deletePasskey(passkey.id);
			toast.success(t('security-passkeys-removed', { name: passkey.name }));
			await tabSync.refresh(SECURITY);
		} catch (error) {
			toast.error(errorMessage(error));
		}
	}
</script>

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('security-passkeys-title')}</Card.Title>
		<Card.Description>
			{mfaEnabled
				? t('security-passkeys-description')
				: t('security-passkeys-description-enables-two-step')}
		</Card.Description>
	</Card.Header>
	{#if passkeys.length > 0}
		<Card.Content>
			<ul class="divide-y">
				{#each passkeys as passkey (passkey.id)}
					<li class="flex items-center gap-3 py-2.5 first:pt-0 last:pb-0">
						<div class="flex size-8 shrink-0 items-center justify-center rounded-xl bg-muted">
							<KeyRoundIcon class="size-4 text-muted-foreground" />
						</div>
						<div class="flex min-w-0 flex-1 flex-col">
							<span class="truncate text-sm font-medium">{passkey.name}</span>
							<span class="text-xs text-muted-foreground">
								{passkey.lastUsedAt
									? t('security-passkeys-used', {
											added: formatDate(passkey.createdAt),
											used: formatRelative(passkey.lastUsedAt)
										})
									: t('security-passkeys-never-used', { added: formatDate(passkey.createdAt) })}
							</span>
						</div>
						<Button
							variant="ghost"
							size="sm"
							aria-label={t('security-passkeys-rename-label', { name: passkey.name })}
							onclick={() => startRename(passkey)}
						>
							{t('security-passkeys-rename')}
						</Button>
						<Button
							variant="ghost"
							size="sm"
							aria-label={t('security-passkeys-remove-label', { name: passkey.name })}
							onclick={() => ((removing = passkey), (removeOpen = true))}
						>
							{t('security-passkeys-remove')}
						</Button>
					</li>
				{/each}
			</ul>
		</Card.Content>
	{/if}
	<Card.Footer class="justify-end">
		{#if supported}
			<Button variant="outline" size="sm" disabled={adding} onclick={add}>
				{#if adding}<Spinner />{:else}<PlusIcon />{/if}
				{t('security-passkeys-add')}
			</Button>
		{:else}
			<p class="text-xs text-muted-foreground">{t('security-passkeys-unsupported')}</p>
		{/if}
	</Card.Footer>
</Card.Root>

<Dialog.Root
	open={renaming !== null}
	onOpenChange={(open) => {
		if (!open) renaming = null;
	}}
>
	<Dialog.Content>
		<form onsubmit={rename} class="contents" novalidate>
			<Dialog.Header>
				<Dialog.Title>{t('security-passkeys-rename-title')}</Dialog.Title>
			</Dialog.Header>
			<Field.Group>
				<FormError message={renameForm.error} />
				<FormField
					id="passkey-name"
					label={t('security-passkeys-name')}
					error={renameForm.fieldErrors.name}
				>
					<Input
						id="passkey-name"
						maxlength={MAX_PASSKEY_NAME_LENGTH}
						required
						bind:value={name}
						oninput={() => renameForm.clearField('name')}
						{...invalid('passkey-name', renameForm.fieldErrors.name)}
					/>
				</FormField>
			</Field.Group>
			<Dialog.Footer>
				<Button type="submit" disabled={renameForm.pending}>
					{#if renameForm.pending}<Spinner />{/if}
					{t('security-passkeys-save')}
				</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>

<ConfirmDialog
	bind:open={removeOpen}
	title={removing ? t('security-passkeys-remove-title', { name: removing.name }) : ''}
	description={mfaEnabled
		? t('security-passkeys-remove-description-two-step')
		: t('security-passkeys-remove-description')}
	confirmLabel={t('security-passkeys-remove')}
	onconfirm={remove}
/>

<RecoveryCodesDialog bind:codes={recoveryCodes} />
