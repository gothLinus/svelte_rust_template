<script lang="ts">
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { t } from '$lib/i18n';
	import { activeItem, navigationFor, tabBarItems } from '$lib/helpers/navigation';
	import type { SignedIn } from '$lib/api';

	/**
	 * The tab bar phones navigate with. The sidebar replaces it from `md` up. Items beyond
	 * what fits go into a "More" menu.
	 */
	let { me }: { me: SignedIn } = $props();

	const bar = $derived(tabBarItems(navigationFor(me).flatMap((group) => group.items)));
	const active = $derived(activeItem(page.url.pathname)?.item);
	const moreActive = $derived(bar.more.some((item) => item.href === active?.href));

	const tab =
		'group flex h-16 w-full flex-col items-center justify-center gap-1 text-xs font-medium text-muted-foreground transition-colors outline-none aria-[current=page]:text-foreground';
	const pill =
		'flex h-8 w-14 items-center justify-center rounded-full transition-colors group-focus-visible:ring-2 group-focus-visible:ring-ring group-active:bg-muted group-aria-[current=page]:bg-muted';
</script>

<nav
	aria-label={t('nav-main')}
	class="fixed inset-x-0 bottom-0 z-20 border-t bg-background/90 pr-[env(safe-area-inset-right)] pb-[env(safe-area-inset-bottom)] pl-[env(safe-area-inset-left)] backdrop-blur md:hidden"
>
	<ul class="mx-auto flex max-w-lg">
		{#each bar.tabs as item (item.href)}
			<li class="flex-1">
				<a
					href={resolve(item.href)}
					aria-current={active?.href === item.href ? 'page' : undefined}
					class={tab}
				>
					<span class={pill}><item.icon class="size-5" /></span>
					{item.title}
				</a>
			</li>
		{/each}
		{#if bar.more.length > 0}
			<li class="flex-1">
				<DropdownMenu.Root>
					<DropdownMenu.Trigger class={tab} aria-current={moreActive ? 'page' : undefined}>
						<span class={pill}><EllipsisIcon class="size-5" /></span>
						{t('nav-more')}
					</DropdownMenu.Trigger>
					<DropdownMenu.Content side="top" align="end" class="w-48">
						{#each bar.more as item (item.href)}
							<DropdownMenu.Item
								onSelect={() => goto(resolve(item.href))}
								aria-current={active?.href === item.href ? 'page' : undefined}
							>
								<item.icon />
								{item.title}
							</DropdownMenu.Item>
						{/each}
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			</li>
		{/if}
	</ul>
</nav>
