import LayoutDashboardIcon from '@lucide/svelte/icons/layout-dashboard';
import NotebookPenIcon from '@lucide/svelte/icons/notebook-pen';
import ScrollTextIcon from '@lucide/svelte/icons/scroll-text';
import ShieldCheckIcon from '@lucide/svelte/icons/shield-check';
import UserRoundIcon from '@lucide/svelte/icons/user-round';
import UsersIcon from '@lucide/svelte/icons/users';
import type { Component } from 'svelte';
import type { Pathname } from '$app/types';
import { hasPermission } from '$lib/auth/permissions';
import { t } from '$lib/i18n';
import type { SignedIn } from '$lib/api';
import { Permission } from '$lib/types/api';

export type NavItem = {
	title: string;
	href: Pathname;
	icon: Component;
	/** Only shown to users with this permission. The server enforces it either way. */
	permission?: Permission;
};

export type NavGroup = { label: string; items: NavItem[] };

function page(
	title: () => string,
	href: Pathname,
	icon: Component,
	permission?: Permission
): NavItem {
	return {
		get title() {
			return title();
		},
		href,
		icon,
		permission
	};
}

function group(label: () => string, items: NavItem[]): NavGroup {
	return {
		get label() {
			return label();
		},
		items
	};
}

export const navigation: NavGroup[] = [
	group(
		() => t('nav-workspace'),
		[
			page(() => t('nav-dashboard'), '/dashboard', LayoutDashboardIcon),
			page(() => t('notes-nav'), '/notes', NotebookPenIcon, Permission.NOTES_READ)
		]
	),
	group(
		() => t('nav-account'),
		[
			page(() => t('nav-profile'), '/settings/profile', UserRoundIcon),
			page(() => t('nav-security'), '/settings/security', ShieldCheckIcon)
		]
	),
	group(
		() => t('nav-administration'),
		[
			page(() => t('nav-users'), '/admin/users', UsersIcon, Permission.USERS_READ),
			page(() => t('nav-audit'), '/admin/audit', ScrollTextIcon, Permission.AUDIT_READ)
		]
	)
];

export function navigationFor(me: SignedIn): NavGroup[] {
	return navigation
		.map((group) => ({
			...group,
			items: group.items.filter((item) => !item.permission || hasPermission(me, item.permission))
		}))
		.filter((group) => group.items.length > 0);
}

export function activeItem(pathname: string): { group: NavGroup; item: NavItem } | undefined {
	for (const group of navigation) {
		const item = group.items.find(
			(item) => pathname === item.href || pathname.startsWith(`${item.href}/`)
		);
		if (item) return { group, item };
	}
	return undefined;
}

export const MAX_TABS = 4;

/**
 * The tab bar's items: all of them when they fit, otherwise the first ones plus a "More"
 * menu with the rest, so the bar keeps `max` tabs however many features there are.
 */
export function tabBarItems(
	items: readonly NavItem[],
	max = MAX_TABS
): { tabs: NavItem[]; more: NavItem[] } {
	if (items.length <= max) return { tabs: [...items], more: [] };
	return { tabs: items.slice(0, max - 1), more: items.slice(max - 1) };
}
