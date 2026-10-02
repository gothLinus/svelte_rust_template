<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { api, errorMessage } from '$lib/api';
	import SessionItem from '$lib/components/security/session-item.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { Spinner } from '$lib/components/ui/spinner';
	import type { Session, User } from '$lib/types/api';
	import { describeUserAgent } from '$lib/helpers/format';
	import { t } from '$lib/i18n';

	/**
	 * Someone else's signed-in devices, for admins. Loads them each time it opens; with
	 * `canManage` each one can be signed out.
	 */
	let {
		open = $bindable(false),
		user,
		canManage
	}: {
		open?: boolean;
		user: User | null;
		canManage: boolean;
	} = $props();

	let sessions = $state<Session[] | null>(null);
	let error = $state<string | null>(null);
	let busy = $state(false);

	async function load(target: User) {
		sessions = null;
		error = null;
		try {
			sessions = await api.admin.sessions(target.id);
		} catch (failure) {
			error = errorMessage(failure);
		}
	}

	$effect(() => {
		if (open && user) void load(user);
	});

	async function revoke(session: Session) {
		if (!user) return;
		busy = true;
		try {
			await api.admin.revokeSession(user.id, session.id);
			sessions = sessions?.filter((each) => each.id !== session.id) ?? null;
			toast.success(
				t('admin-session-signed-out', {
					name: user.username,
					device: describeUserAgent(session.userAgent)
				})
			);
		} catch (failure) {
			toast.error(errorMessage(failure));
		} finally {
			busy = false;
		}
	}
</script>

<Dialog.Root bind:open>
	<Dialog.Content class="sm:max-w-lg">
		<Dialog.Header>
			<Dialog.Title>{t('admin-sessions-title', { name: user?.username ?? '' })}</Dialog.Title>
			<Dialog.Description>{t('admin-sessions-description')}</Dialog.Description>
		</Dialog.Header>
		<div aria-live="polite" aria-busy={sessions === null && error === null}>
			{#if error}
				<p class="text-sm text-destructive">{error}</p>
			{:else if sessions === null}
				<div class="flex justify-center py-6"><Spinner /></div>
			{:else if sessions.length === 0}
				<p class="py-6 text-center text-sm text-muted-foreground">{t('admin-sessions-empty')}</p>
			{:else}
				<ul class="divide-y">
					{#each sessions as session (session.id)}
						<SessionItem {session} {busy} onrevoke={canManage ? revoke : undefined} />
					{/each}
				</ul>
			{/if}
		</div>
	</Dialog.Content>
</Dialog.Root>
