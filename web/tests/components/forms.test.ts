import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import ForgotPassword from '../../src/routes/(public)/(guest)/forgot-password/+page.svelte';
import Register from '../../src/routes/(public)/(guest)/register/+page.svelte';
import { t } from '$lib/i18n';
import { MeSchema, VerificationPendingSchema } from '$lib/types/api';
import { me, mockFetch, problem, reply, rootData } from '../helpers';
import { navigation, page, visit } from './fake-app.svelte';

describe('FormField and FormState', () => {
	it('ties the error to the input and focuses the first invalid field', async () => {
		const user = userEvent.setup();
		render(ForgotPassword);

		await user.click(screen.getByRole('button', { name: t('forgot-submit') }));

		const email = screen.getByLabelText(t('forgot-email'));
		expect(email).toHaveAttribute('aria-invalid', 'true');
		expect(email).toHaveAccessibleDescription(t('validation-required'));
		expect(email).toHaveFocus();

		await user.type(email, 'a');
		expect(email).not.toHaveAttribute('aria-invalid');
	});

	it('shows server errors for fields the form does not have as the form error', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(
				problem(422, 'validation_failed', {
					errors: [{ field: 'locale', code: 'invalid', message: 'is not supported' }]
				})
			)
		);
		render(ForgotPassword);

		await user.type(screen.getByLabelText(t('forgot-email')), 'alice@example.com');
		await user.click(screen.getByRole('button', { name: t('forgot-submit') }));

		expect(await screen.findByRole('alert')).toHaveTextContent(
			t('form-field-problem', { field: 'Locale', message: 'is not supported' })
		);
	});

	it('confirms without telling whether the address has an account', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(new Response(null, { status: 202 }));
		vi.stubGlobal('fetch', fetchFn);
		render(ForgotPassword);

		await user.type(screen.getByLabelText(t('forgot-email')), ' alice@example.com ');
		await user.click(screen.getByRole('button', { name: t('forgot-submit') }));

		expect(
			await screen.findByRole('heading', { name: t('forgot-sent-title') })
		).toBeInTheDocument();
		expect(screen.getByText('alice@example.com')).toBeInTheDocument();
		expect(fetchFn).toHaveBeenCalledOnce();
	});

	it('says how long the link works as the server is configured', async () => {
		const user = userEvent.setup();
		vi.stubGlobal('fetch', mockFetch(new Response(null, { status: 202 })));
		page.data = rootData(null, { lifetimes: { passwordResetMinutes: 45 } });
		render(ForgotPassword);

		await user.type(screen.getByLabelText(t('forgot-email')), 'alice@example.com');
		await user.click(screen.getByRole('button', { name: t('forgot-submit') }));

		await screen.findByRole('heading', { name: t('forgot-sent-title') });
		expect(document.body).toHaveTextContent(
			t('forgot-sent-description', { email: 'alice@example.com', minutes: 45 })
		);
	});

	it('describes a field with its description until there is an error', async () => {
		const user = userEvent.setup();
		render(Register, { data: rootData() });

		const username = screen.getByLabelText(t('register-username'));
		expect(username).toHaveAccessibleDescription(t('register-username-hint'));

		await user.click(screen.getByRole('button', { name: t('register-submit') }));
		expect(username).toHaveAccessibleDescription(t('validation-required'));
		expect(username).toHaveFocus();
	});
});

describe('registration', () => {
	it('signs in and continues where the visitor was headed', async () => {
		const user = userEvent.setup();
		visit('/register?redirectTo=%2Fnotes%3Fscope%3Dall');
		const fetchFn = mockFetch(reply(MeSchema, me(), 201));
		vi.stubGlobal('fetch', fetchFn);
		render(Register, { data: rootData() });

		expect(screen.getByRole('link', { name: t('register-sign-in') })).toHaveAttribute(
			'href',
			'/login?redirectTo=%2Fnotes%3Fscope%3Dall'
		);

		await user.type(screen.getByLabelText(t('register-username')), 'alice');
		await user.type(screen.getByLabelText(t('register-email')), 'alice@example.com');
		await user.type(screen.getByLabelText(t('register-password')), 'correct horse');
		await user.type(screen.getByLabelText(t('register-confirmation')), 'correct horse');
		await user.click(screen.getByRole('button', { name: t('register-submit') }));

		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/notes?scope=all', { invalidateAll: true })
		);
	});

	it('asks to verify the address when the server wants that first', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(reply(VerificationPendingSchema, { email: 'alice@example.com' }, 202))
		);
		render(Register, { data: rootData() });

		await user.type(screen.getByLabelText(t('register-username')), 'alice');
		await user.type(screen.getByLabelText(t('register-email')), 'alice@example.com');
		await user.type(screen.getByLabelText(t('register-password')), 'correct horse');
		await user.type(screen.getByLabelText(t('register-confirmation')), 'correct horse');
		await user.click(screen.getByRole('button', { name: t('register-submit') }));

		expect(
			await screen.findByRole('heading', { name: t('register-pending-title') })
		).toBeInTheDocument();
		expect(navigation.goto).not.toHaveBeenCalled();
	});
});
