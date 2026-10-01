<script lang="ts">
	import './layout.css';
	import { ModeWatcher } from 'mode-watcher';
	import { toast } from 'svelte-sonner';
	import { onMount } from 'svelte';
	import { goto, invalidate } from '$app/navigation';
	import { page } from '$app/state';
	import favicon from '$lib/assets/favicon.ico';
	import { setErrorListener, setReauthenticator, setUnauthenticatedHandler } from '$lib/api';
	import { SESSION, handleApiError, loginUrl, reauth, session, tabSync } from '$lib/auth';
	import ReauthDialog from '$lib/components/reauth/reauth-dialog.svelte';
	import SessionErrorBanner from '$lib/components/session-error-banner.svelte';
	import { t } from '$lib/i18n';
	import { followAccountLanguage } from '$lib/helpers/language';
	import { site } from '$lib/helpers/site.svelte';
	import { Toaster } from '$lib/components/ui/sonner';

	let { data, children } = $props();

	session.connect(() => data.me);
	site.connect(() => data.methods.appName);

	$effect(() => {
		void followAccountLanguage(data.me?.user.locale);
	});

	let ending = false;
	setUnauthenticatedHandler(() => {
		if (ending || !session.isAuthenticated) return;
		ending = true;
		toast.info(t('app-session-ended'));
		// `loginUrl` returns a same-site path.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(loginUrl(page.url), { invalidateAll: true }).finally(() => (ending = false));
	});

	// Follow what the other tabs change (an email verified from a mailed link, signing in
	// or out), and re-check the session when this tab comes back into view.
	onMount(() => tabSync.listen());

	setReauthenticator(() => reauth.request());

	setErrorListener(
		handleApiError({
			signedIn: () => session.isAuthenticated,
			refreshSession: () => invalidate(SESSION)
		})
	);
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>

<!-- `static/theme.js` applies the saved mode before the first paint instead. -->
<ModeWatcher disableHeadScriptInjection />
<Toaster richColors position="top-center" />

{#if data.sessionError && !page.error}
	<SessionErrorBanner message={data.sessionError} />
{/if}

{@render children()}

<ReauthDialog />
