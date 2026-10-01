import { toast } from 'svelte-sonner';
import { api, errorMessage } from '$lib/api';
import { SESSION, session, tabSync } from '$lib/auth';
import { i18n } from '$lib/i18n';

/** A language by its own name, such as "Deutsch" for `de`, so readers of it can find it. */
export function languageName(tag: string): string {
	try {
		const name = new Intl.DisplayNames([tag], { type: 'language' }).of(tag) ?? tag;
		return name.charAt(0).toLocaleUpperCase(tag) + name.slice(1);
	} catch {
		return tag;
	}
}

/**
 * Switches the app to `tag` and remembers it on this device. Signed in, the account keeps
 * it too, so mail and texts follow and other devices switch on their next visit.
 */
export async function chooseLanguage(tag: string): Promise<boolean> {
	const chosen = await i18n.choose(tag);
	if (!session.isAuthenticated || session.user?.locale === chosen) return true;
	try {
		await api.me.setLocale(chosen);
		await tabSync.refresh(SESSION);
		return true;
	} catch (error) {
		toast.error(errorMessage(error));
		return false;
	}
}

/**
 * Follows the account's language once the session says it differs, e.g. after signing in on
 * a device that showed another one. A language the app no longer has is ignored.
 */
export async function followAccountLanguage(locale: string | undefined): Promise<void> {
	if (!locale || locale === i18n.locale || !i18n.locales.includes(locale)) return;
	await i18n.choose(locale);
}
