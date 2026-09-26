declare global {
	namespace App {
		interface PageData {
			me: import('$lib/api').SignedIn | null;
			sessionError: string | null;
			methods: import('$lib/types/api').AuthMethods;
		}
	}
}

export {};
