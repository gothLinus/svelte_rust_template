import { t } from '$lib/i18n';

export interface FieldError {
	field: string;
	code: string;
	message: string;
}

/**
 * The body of every failed response: an RFC 9457 problem document
 * (`application/problem+json`). Errors are JSON, unlike the protobuf success bodies, so
 * proxies and logs can read them. Mirrors `application::problem::ProblemDetails`.
 */
export interface ProblemDetails {
	type: string;
	title: string;
	status: number;
	detail?: string;
	instance?: string;
	code: string;
	errors?: FieldError[];
}

export const NETWORK_ERROR = 'network_error';
export const UNKNOWN_ERROR = 'unknown_error';
export const TIMEOUT_ERROR = 'timeout';
export const ABORTED = 'aborted';
export const INVALID_RESPONSE = 'invalid_response';
export const REAUTH_REQUIRED = 'reauth_required';

/**
 * Messages for codes where the server's own wording does not say what to do next; `undefined`
 * for every other code, which shows the server's message (worded in the language the client
 * asked for).
 */
function codeMessage(code: string): string | undefined {
	switch (code) {
		case 'email_unverified':
			return t('error-email-unverified');
		case REAUTH_REQUIRED:
			return t('error-reauth-required');
		default:
			return undefined;
	}
}

/**
 * A failed API call. Built from the server's RFC 9457 problem document when there is one,
 * so `code` is the stable, machine-readable reason: switch on it, never on `message`.
 */
export class ApiError extends Error {
	readonly status: number;
	readonly code: string;
	readonly fieldErrors: readonly FieldError[];
	readonly retryAfter: number | null;

	constructor(
		status: number,
		code: string,
		message: string,
		fieldErrors: readonly FieldError[] = [],
		retryAfter: number | null = null
	) {
		super(message);
		this.name = 'ApiError';
		this.status = status;
		this.code = code;
		this.fieldErrors = fieldErrors;
		this.retryAfter = retryAfter;
	}

	static fromProblem(problem: ProblemDetails, retryAfter: number | null = null): ApiError {
		return new ApiError(
			problem.status,
			problem.code,
			problem.detail ?? problem.title,
			problem.errors ?? [],
			retryAfter
		);
	}

	static network(): ApiError {
		return new ApiError(0, NETWORK_ERROR, t('error-network'));
	}

	static timeout(): ApiError {
		return new ApiError(0, TIMEOUT_ERROR, t('error-timeout'));
	}

	static aborted(): ApiError {
		return new ApiError(0, ABORTED, t('error-aborted'));
	}

	static invalidResponse(status: number): ApiError {
		return new ApiError(status, INVALID_RESPONSE, t('error-invalid-response'));
	}

	get isValidation(): boolean {
		return this.status === 422 && this.fieldErrors.length > 0;
	}

	get isAborted(): boolean {
		return this.code === ABORTED;
	}

	fieldError(field: string): string | undefined {
		return this.fieldErrors.find((error) => error.field === field)?.message;
	}
}

function isRecord(value: unknown): value is Record<string, unknown> {
	return typeof value === 'object' && value !== null;
}

function isFieldError(value: unknown): value is FieldError {
	return (
		isRecord(value) &&
		typeof value.field === 'string' &&
		typeof value.code === 'string' &&
		typeof value.message === 'string'
	);
}

export function isProblemDetails(value: unknown): value is ProblemDetails {
	return (
		isRecord(value) &&
		typeof value.status === 'number' &&
		typeof value.code === 'string' &&
		typeof value.title === 'string' &&
		(value.detail === undefined || typeof value.detail === 'string') &&
		(value.errors === undefined ||
			(Array.isArray(value.errors) && value.errors.every(isFieldError)))
	);
}

export function errorMessage(error: unknown): string {
	if (error instanceof ApiError) return codeMessage(error.code) ?? error.message;
	return t('error-unknown');
}
