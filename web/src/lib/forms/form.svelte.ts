import { tick } from 'svelte';
import { ApiError, errorMessage } from '$lib/api';
import { t } from '$lib/i18n';

export type FieldErrors<F extends string> = Partial<Record<F, string>>;

export interface SubmitOptions<F extends string> {
	validation?: FieldErrors<F>;
}

/**
 * The state of one form: whether it is submitting, the form-level error, and per-field
 * errors, from client-side validation or from the server's `422` response.
 *
 * ```svelte
 * const form = new FormState(['email', 'password']);
 * const me = await form.submit(() => api.auth.login({ email, password }), {
 *   validation: validate({ email, password }, { email: rules.email, password: rules.required })
 * });
 * ```
 *
 * After a failed submit, focus moves to the first invalid field of the submitted form.
 */
export class FormState<F extends string = string> {
	pending = $state(false);
	error = $state<string | null>(null);
	fieldErrors = $state<FieldErrors<F>>({});

	readonly hasErrors = $derived(
		this.error !== null || Object.values(this.fieldErrors).some(Boolean)
	);

	readonly #fields: readonly string[];

	constructor(fields: readonly F[]) {
		this.#fields = fields;
	}

	/**
	 * Validates, then runs `action`. Resolves to its result, or to `undefined` when
	 * validation failed, `action` threw, or a submission is still running; the errors are
	 * then on the form.
	 */
	async submit<T>(
		action: () => Promise<T>,
		options: SubmitOptions<F> = {}
	): Promise<T | undefined> {
		// A double click or a second Enter must not send the request twice.
		if (this.pending) return undefined;
		const form = submittedForm();

		this.error = null;
		this.fieldErrors = { ...options.validation };
		if (Object.values(this.fieldErrors).some(Boolean)) {
			void focusFirstInvalid(form);
			return undefined;
		}

		this.pending = true;
		try {
			return await action();
		} catch (error) {
			this.fail(error);
			void focusFirstInvalid(form);
			return undefined;
		} finally {
			this.pending = false;
		}
	}

	/**
	 * Shows `error` on the form: field errors for a `422`, a message otherwise. Errors for
	 * fields the form does not render become the form's message, so none is lost.
	 */
	fail(error: unknown): void {
		if (error instanceof ApiError && error.isAborted) return;
		if (!(error instanceof ApiError && error.isValidation)) {
			this.error = errorMessage(error);
			return;
		}
		const fieldErrors: FieldErrors<F> = {};
		const others: string[] = [];
		for (const { field, message } of error.fieldErrors) {
			// The server names fields by the request's camelCase keys; forms use the same.
			if (this.#fields.includes(field)) fieldErrors[field as F] ??= message;
			else others.push(t('form-field-problem', { field: fieldLabel(field), message }));
		}
		this.fieldErrors = fieldErrors;
		this.error = others.length > 0 ? others.join(' ') : null;
	}

	clearField(field: F): void {
		if (this.fieldErrors[field]) this.fieldErrors = { ...this.fieldErrors, [field]: undefined };
	}

	reset(): void {
		this.pending = false;
		this.error = null;
		this.fieldErrors = {};
	}
}

function fieldLabel(field: string): string {
	const words = field.replace(/([a-z0-9])([A-Z])/g, '$1 $2').toLowerCase();
	return words.charAt(0).toUpperCase() + words.slice(1);
}

function submittedForm(): HTMLFormElement | null {
	if (typeof document === 'undefined') return null;
	return document.activeElement?.closest('form') ?? null;
}

/** Moves focus to the first field marked invalid, once the errors have rendered. */
async function focusFirstInvalid(form: HTMLFormElement | null): Promise<void> {
	if (!form) return;
	await tick();
	form.querySelector<HTMLElement>('[aria-invalid="true"]')?.focus();
}
