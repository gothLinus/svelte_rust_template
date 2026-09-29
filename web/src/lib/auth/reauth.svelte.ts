/**
 * The "Confirm it's you" prompt for step-up re-authentication, as state the root layout's
 * `ReauthDialog` renders. `ApiClient` asks for it (see `setReauthenticator`) when a
 * sensitive request answers `403 reauth_required`, then retries the request once.
 */
export class ReauthPrompt {
	open = $state(false);
	#resolve: ((done: boolean) => void) | null = null;

	request(): Promise<boolean> {
		this.#resolve?.(false);
		this.open = true;
		return new Promise((resolve) => (this.#resolve = resolve));
	}

	finish(done: boolean): void {
		this.open = false;
		this.#resolve?.(done);
		this.#resolve = null;
	}
}

export const reauth = new ReauthPrompt();
