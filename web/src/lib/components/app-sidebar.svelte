<script lang="ts">
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import Logo from '$lib/components/logo.svelte';
	import NavUser from '$lib/components/nav-user.svelte';
	import * as Sidebar from '$lib/components/ui/sidebar';
	import type { SignedIn } from '$lib/api';
	import { t } from '$lib/i18n';
	import { activeItem, navigationFor } from '$lib/helpers/navigation';

	let { me }: { me: SignedIn } = $props();

	const groups = $derived(navigationFor(me));
	const active = $derived(activeItem(page.url.pathname)?.item);
</script>

<Sidebar.Root collapsible="icon">
	<Sidebar.Header>
		<Sidebar.Menu>
			<Sidebar.MenuItem>
				<Sidebar.MenuButton size="lg">
					{#snippet child({ props })}
						<a href={resolve('/dashboard')} {...props}
							><Logo /><span class="sr-only">{t('nav-logo-link')}</span></a
						>
					{/snippet}
				</Sidebar.MenuButton>
			</Sidebar.MenuItem>
		</Sidebar.Menu>
	</Sidebar.Header>

	<Sidebar.Content>
		<nav aria-label={t('nav-main')} class="contents">
			{#each groups as group (group.label)}
				<Sidebar.Group>
					<Sidebar.GroupLabel>{group.label}</Sidebar.GroupLabel>
					<Sidebar.Menu>
						{#each group.items as item (item.href)}
							<Sidebar.MenuItem>
								<Sidebar.MenuButton
									isActive={active?.href === item.href}
									tooltipContent={item.title}
								>
									{#snippet child({ props })}
										<a
											href={resolve(item.href)}
											aria-current={active?.href === item.href ? 'page' : undefined}
											{...props}
										>
											<item.icon />
											<span>{item.title}</span>
										</a>
									{/snippet}
								</Sidebar.MenuButton>
							</Sidebar.MenuItem>
						{/each}
					</Sidebar.Menu>
				</Sidebar.Group>
			{/each}
		</nav>
	</Sidebar.Content>

	<Sidebar.Footer>
		<NavUser user={me.user} />
	</Sidebar.Footer>
	<Sidebar.Rail />
</Sidebar.Root>
