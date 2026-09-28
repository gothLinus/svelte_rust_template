/**
 * Remembers which page led to which, so keyset pagination (which only knows "the page
 * after this cursor") can also go back.
 *
 * Each cursor maps to the cursor of the page before it, so what "Previous" does depends
 * only on the page in the URL: Back and Forward in the browser keep it right. The map
 * lives in the page component; a list opened from a link (or reloaded) goes back to its
 * first page.
 */
export class CursorTrail {
	readonly #previous = new Map<string, string | null>();

	hasPrevious(current: string | null): boolean {
		return current !== null;
	}

	next(current: string | null, next: string): string {
		this.#previous.set(next, current);
		return next;
	}

	previous(current: string | null): string | null {
		return current === null ? null : (this.#previous.get(current) ?? null);
	}

	reset(): void {
		this.#previous.clear();
	}
}
