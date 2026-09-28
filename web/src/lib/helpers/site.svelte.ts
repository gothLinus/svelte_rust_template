import { FALLBACK_APP_NAME } from '$lib/auth/methods';
import { t } from '$lib/i18n';

/**
 * The app's name and tagline, for titles, the logo and downloads. The name is the server's
 * `APP_NAME`, which the root layout loads and connects here, so it is set in one place.
 * Mail uses `MAIL_FROM` from the server's `.env`.
 */
class Site {
	#name = $state<() => string>(() => FALLBACK_APP_NAME);

	readonly name = $derived(this.#name());
	get description(): string {
		return t('site-description');
	}

	connect(source: () => string): void {
		this.#name = source;
	}
}

export const site = new Site();
