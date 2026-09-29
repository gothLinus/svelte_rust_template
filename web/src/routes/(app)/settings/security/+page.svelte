<script lang="ts">
	import { onMount } from 'svelte';
	import { toast } from 'svelte-sonner';
	import { replaceState } from '$app/navigation';
	import { page } from '$app/state';
	import { oauthLinkUrl } from '$lib/api';
	import { oauthErrorMessage, reauth } from '$lib/auth';
	import { t } from '$lib/i18n';
	import PageHeading from '$lib/components/page-heading.svelte';
	import LinkedAccountsCard, {
		takeLinkingProvider
	} from '$lib/components/security/linked-accounts-card.svelte';
	import PasskeysCard from '$lib/components/security/passkeys-card.svelte';
	import PasswordCard from '$lib/components/security/password-card.svelte';
	import SessionsCard from '$lib/components/security/sessions-card.svelte';
	import TwoStepCard from '$lib/components/security/two-step-card.svelte';

	let { data } = $props();

	onMount(() => {
		const params = page.url.searchParams;
		const code = params.get('error');
		const linked = params.get('linked');
		const provider = takeLinkingProvider();
		if (code || linked) replaceState(page.url.pathname, page.state);

		if (code === 'reauth_required' && provider) {
			void relink(provider);
			return;
		}
		const error = oauthErrorMessage(code);
		if (error) toast.error(error);
		if (linked) {
			const name = data.methods.providers.find((p) => p.id === linked)?.name ?? linked;
			toast.success(t('settings-security-linked', { provider: name }));
		}
	});

	/** Linking needed a recent sign-in: confirm it is the user, then start it once more. */
	async function relink(provider: string) {
		if (await reauth.request()) window.location.assign(oauthLinkUrl(provider));
		else toast.error(oauthErrorMessage('reauth_required') ?? '');
	}
</script>

<PageHeading
	title={t('settings-security-title')}
	description={t('settings-security-description')}
/>

<PasswordCard email={data.me.user.email} hasPassword={data.security.hasPassword} />
<TwoStepCard security={data.security} />
<PasskeysCard passkeys={data.security.passkeys} mfaEnabled={data.security.mfaEnabled} />
<LinkedAccountsCard providers={data.methods.providers} linked={data.security.linkedAccounts} />
<SessionsCard sessions={data.sessions} />
