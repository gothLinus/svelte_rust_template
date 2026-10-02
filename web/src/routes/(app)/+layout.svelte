<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import AppSidebar from '$lib/components/app-sidebar.svelte';
	import BottomNav from '$lib/components/bottom-nav.svelte';
	import EmailVerificationBanner from '$lib/components/email-verification-banner.svelte';
	import Logo from '$lib/components/logo.svelte';
	import LanguageMenu from '$lib/components/language-menu.svelte';
	import ThemeToggle from '$lib/components/theme-toggle.svelte';
	import UserMenu from '$lib/components/user-menu.svelte';
	import * as Breadcrumb from '$lib/components/ui/breadcrumb';
	import { Separator } from '$lib/components/ui/separator';
	import * as Sidebar from '$lib/components/ui/sidebar';
	import { activeItem } from '$lib/helpers/navigation';
	import { t } from '$lib/i18n';
	import { site } from '$lib/helpers/site.svelte';

	let { data, children } = $props();

	const active = $derived(activeItem(page.url.pathname));

	let content = $state<HTMLElement | null>(null);

	// Past the header's controls, straight to the page's own content.
	function skipToContent(event: MouseEvent) {
		event.preventDefault();
		content?.focus();
	}
</script>

<svelte:head>
	<title
		>{active ? t('app-page-title', { page: active.item.title, site: site.name }) : site.name}</title
	>
</svelte:head>

<a
	href="#content"
	onclick={skipToContent}
	class="sr-only z-50 rounded-xl bg-background px-3 py-2 text-sm font-medium shadow-sm focus:not-sr-only focus:fixed focus:top-2 focus:left-2"
>
	{t('app-skip-to-content')}
</a>

<Sidebar.Provider>
	<AppSidebar me={data.me} />
	<Sidebar.Inset>
		<header
			class="sticky top-0 z-10 flex h-14 shrink-0 items-center gap-2 border-b bg-background/90 px-4 backdrop-blur"
		>
			<a
				href={resolve('/dashboard')}
				class="-ml-1 rounded-xl md:hidden"
				aria-label={t('app-dashboard-link', { site: site.name })}
			>
				<Logo withName={false} />
			</a>
			<Sidebar.Trigger class="-ml-1 hidden md:inline-flex" />
			<Separator
				orientation="vertical"
				class="mr-1 hidden md:block data-vertical:h-4 data-vertical:self-center"
			/>
			{#if active}
				<Breadcrumb.Root class="min-w-0">
					<Breadcrumb.List>
						<Breadcrumb.Item class="hidden md:inline-flex">{active.group.label}</Breadcrumb.Item>
						<Breadcrumb.Separator class="hidden md:block" />
						<Breadcrumb.Item class="min-w-0">
							<Breadcrumb.Page class="truncate">{active.item.title}</Breadcrumb.Page>
						</Breadcrumb.Item>
					</Breadcrumb.List>
				</Breadcrumb.Root>
			{/if}
			<div class="ml-auto flex items-center gap-1">
				<LanguageMenu />
				<ThemeToggle />
				<UserMenu user={data.me.user} class="-mr-1 md:hidden" />
			</div>
		</header>
		<!-- `Sidebar.Inset` is the page's <main>. The bottom padding clears the tab bar. -->
		<div
			id="content"
			tabindex="-1"
			bind:this={content}
			class="mx-auto flex w-full max-w-4xl flex-1 flex-col gap-6 px-4 pt-4 pb-[calc(--spacing(20)+env(safe-area-inset-bottom))] md:p-8"
		>
			{#if !data.me.user.emailVerified}
				<EmailVerificationBanner email={data.me.user.email} />
			{/if}
			{@render children()}
		</div>
	</Sidebar.Inset>
	<BottomNav me={data.me} />
</Sidebar.Provider>
