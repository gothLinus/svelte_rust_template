import { afterEach, describe, expect, it } from 'vitest';
import { ApiError, NETWORK_ERROR, errorMessage, isProblemDetails } from '$lib/api';
import { i18n, t } from '$lib/i18n';
import { useEnglish, useGerman } from '../helpers';

describe('isProblemDetails', () => {
	it('accepts the server shape', () => {
		expect(
			isProblemDetails({ type: 'about:blank', title: 'Not Found', status: 404, code: 'not_found' })
		).toBe(true);
		expect(
			isProblemDetails({
				title: 'Unprocessable Entity',
				status: 422,
				code: 'validation_failed',
				detail: 'invalid',
				errors: [{ field: 'email', code: 'required', message: 'required' }]
			})
		).toBe(true);
	});

	it('rejects anything else', () => {
		for (const value of [
			null,
			'error',
			42,
			{ error: 'x' },
			{ title: 'x', status: '404', code: 'x' },
			{ title: 'x', status: 404 },
			{ title: 'x', status: 404, code: 'x', detail: 1 },
			{ title: 'x', status: 422, code: 'x', errors: 'nope' },
			{ title: 'x', status: 422, code: 'x', errors: [{ field: 'a' }] }
		]) {
			expect(isProblemDetails(value), JSON.stringify(value)).toBe(false);
		}
	});
});

describe('ApiError', () => {
	it('is built from a problem document', () => {
		const error = ApiError.fromProblem({
			type: 'about:blank',
			title: 'Conflict',
			status: 409,
			code: 'email_taken'
		});
		expect(error.message).toBe('Conflict');
		expect(error.fieldErrors).toEqual([]);
		expect(error.isValidation).toBe(false);
		expect(error.name).toBe('ApiError');
	});

	it('describes network failures', () => {
		const error = ApiError.network();
		expect(error.code).toBe(NETWORK_ERROR);
		expect(error.status).toBe(0);
	});
});

describe('errorMessage', () => {
	it('uses the API message and hides everything else', () => {
		expect(errorMessage(new ApiError(409, 'email_taken', 'taken'))).toBe('taken');
		expect(errorMessage(new ApiError(409, 'email_unverified', 'unverified'))).toBe(
			t('error-email-unverified')
		);
		expect(errorMessage(new ApiError(403, 'reauth_required', 'reauth'))).toBe(
			t('error-reauth-required')
		);
		expect(errorMessage(new Error('stack trace details'))).toBe(t('error-unknown'));
	});
});

describe('the words of client-side errors', () => {
	afterEach(useEnglish);

	it('come from the catalog', () => {
		expect(ApiError.network().message).toBe(t('error-network'));
		expect(ApiError.timeout().message).toBe(t('error-timeout'));
		expect(ApiError.aborted().message).toBe(t('error-aborted'));
		expect(ApiError.invalidResponse(200).message).toBe(t('error-invalid-response'));
	});

	it('are in the language in use, English where it has no translation', async () => {
		await useGerman();
		expect(i18n.locale).toBe('de');
		expect(ApiError.network().message).toBe('Der Server ist nicht erreichbar.');
		expect(errorMessage(new Error('x'))).toBe(t('error-unknown'));
		expect(errorMessage(new Error('x'))).toContain('Something went wrong');
	});
});
