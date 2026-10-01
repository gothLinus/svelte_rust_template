<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { api } from '$lib/api';
	import { EVERYTHING, SESSION, tabSync } from '$lib/auth';
	import FormError from '$lib/components/form-error.svelte';
	import FormField from '$lib/components/form-field.svelte';
	import PageHeading from '$lib/components/page-heading.svelte';
	import DataCard from '$lib/components/profile/data-card.svelte';
	import EmailCard from '$lib/components/profile/email-card.svelte';
	import LanguageCard from '$lib/components/profile/language-card.svelte';
	import PhoneCard from '$lib/components/profile/phone-card.svelte';
	import PasswordInput from '$lib/components/password-input.svelte';
	import * as AlertDialog from '$lib/components/ui/alert-dialog';
	import { Button, buttonVariants } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import * as Field from '$lib/components/ui/field';
	import { Input } from '$lib/components/ui/input';
	import { Spinner } from '$lib/components/ui/spinner';
	import { FormState, invalid, rules, validate } from '$lib/forms';
	import { t } from '$lib/i18n';
	import { MAX_USERNAME_LENGTH } from '$lib/forms/validation';
	import { cn } from '$lib/utils';

	let { data } = $props();

	// Seeded once from the loaded user; the form owns the value from then on.
	// svelte-ignore state_referenced_locally
	let username = $state(data.me.user.username);
	const profile = new FormState(['username']);
	const unchanged = $derived(username.trim().toLowerCase() === data.me.user.username);

	async function saveProfile(event: SubmitEvent) {
		event.preventDefault();
		const me = await profile.submit(() => api.me.updateProfile({ username }), {
			validation: validate({ username }, { username: rules.username })
		});
		if (!me) return;

		username = me.user.username;
		await tabSync.refresh(SESSION);
		toast.success(t('settings-profile-updated'));
	}

	let deleteOpen = $state(false);
	let password = $state('');
	const deletion = new FormState(['password']);

	async function deleteAccount(event: SubmitEvent) {
		event.preventDefault();
		const hasPassword = data.me.user.hasPassword;
		const done = await deletion.submit(
			async () => {
				await api.me.deleteAccount(hasPassword ? { password } : {});
				return true;
			},
			{ validation: hasPassword ? validate({ password }, { password: rules.required }) : {} }
		);
		if (!done) return;

		deleteOpen = false;
		toast.success(t('settings-delete-done'));
		tabSync.announce(EVERYTHING);
		await goto(resolve('/'), { invalidateAll: true });
	}

	function deleteOpenChange(open: boolean) {
		if (!open) return;
		password = '';
		deletion.reset();
	}
</script>

<PageHeading title={t('settings-profile-title')} description={t('settings-profile-description')} />

<Card.Root size="sm">
	<Card.Header>
		<Card.Title level={2}>{t('settings-personal-title')}</Card.Title>
		<Card.Description>{t('settings-personal-description')}</Card.Description>
	</Card.Header>
	<form onsubmit={saveProfile} novalidate>
		<Card.Content>
			<Field.Group>
				<FormError message={profile.error} />
				<FormField
					id="username"
					label={t('settings-username')}
					error={profile.fieldErrors.username}
					description={t('settings-username-hint')}
				>
					<Input
						id="username"
						autocomplete="username"
						autocapitalize="off"
						spellcheck={false}
						maxlength={MAX_USERNAME_LENGTH}
						required
						bind:value={username}
						oninput={() => profile.clearField('username')}
						{...invalid('username', profile.fieldErrors.username, { described: true })}
					/>
				</FormField>
			</Field.Group>
		</Card.Content>
		<Card.Footer class="mt-4 justify-end">
			<Button type="submit" size="sm" disabled={profile.pending || unchanged}>
				{#if profile.pending}<Spinner />{/if}
				{t('settings-save')}
			</Button>
		</Card.Footer>
	</form>
</Card.Root>

<LanguageCard />
<EmailCard email={data.me.user.email} verified={data.me.user.emailVerified} />
<PhoneCard user={data.me.user} channels={data.methods.textChannels} />
<DataCard />

<Card.Root size="sm" class="ring-destructive/30">
	<Card.Header>
		<Card.Title level={2}>{t('settings-delete-title')}</Card.Title>
		<Card.Description>
			{t('settings-delete-description')}
		</Card.Description>
	</Card.Header>
	<Card.Footer class="justify-end">
		<AlertDialog.Root bind:open={deleteOpen} onOpenChange={deleteOpenChange}>
			<AlertDialog.Trigger class={cn(buttonVariants({ variant: 'destructive', size: 'sm' }))}>
				{t('settings-delete-open')}
			</AlertDialog.Trigger>
			<AlertDialog.Content>
				<form onsubmit={deleteAccount} class="contents" novalidate>
					<AlertDialog.Header>
						<AlertDialog.Title>{t('settings-delete-dialog-title')}</AlertDialog.Title>
						<AlertDialog.Description>
							{#if data.me.user.hasPassword}
								{t('settings-delete-dialog-password')}
							{:else}
								{t('settings-delete-dialog-no-password')}
							{/if}
						</AlertDialog.Description>
					</AlertDialog.Header>
					<Field.Group>
						<FormError message={deletion.error} />
						{#if data.me.user.hasPassword}
							<FormField
								id="delete-password"
								label={t('settings-delete-password')}
								error={deletion.fieldErrors.password}
							>
								<PasswordInput
									id="delete-password"
									autocomplete="current-password"
									required
									bind:value={password}
									oninput={() => deletion.clearField('password')}
									{...invalid('delete-password', deletion.fieldErrors.password)}
								/>
							</FormField>
						{/if}
					</Field.Group>
					<AlertDialog.Footer>
						<AlertDialog.Cancel type="button" disabled={deletion.pending}
							>{t('common-cancel')}</AlertDialog.Cancel
						>
						<Button type="submit" variant="destructive" disabled={deletion.pending}>
							{#if deletion.pending}<Spinner />{/if}
							{t('settings-delete-submit')}
						</Button>
					</AlertDialog.Footer>
				</form>
			</AlertDialog.Content>
		</AlertDialog.Root>
	</Card.Footer>
</Card.Root>
