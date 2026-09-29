import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { afterEach, describe, expect, it, vi } from 'vitest';
import { api, setReauthenticator } from '$lib/api';
import { reauth } from '$lib/auth';
import { t } from '$lib/i18n';
import ReauthDialog from '$lib/components/reauth/reauth-dialog.svelte';
import CancelEmailChange from '../../src/routes/(public)/cancel-email-change/+page.svelte';
import { create } from '@bufbuild/protobuf';
import {
	CancelEmailChangeRequestSchema,
	ReauthMethod,
	ReauthMethodsSchema,
	ReauthenticateRequestSchema,
	RecoveryCodesSchema
} from '$lib/types/api';
import { callOf, mockFetch, noContent, problem, reply, sent } from '../helpers';
import { navigation, page, visit } from './fake-app.svelte';

const methods = (...methods: ReauthMethod[]) => reply(ReauthMethodsSchema, { methods });
const reauthenticated = (method: ReauthMethod, secret: string) =>
	create(ReauthenticateRequestSchema, { method, secret });

afterEach(() => {
	reauth.finish(false);
	setReauthenticator(null);
});

describe('re-authentication dialog', () => {
	it('confirms with the password, and the original request goes through', async () => {
		const user = userEvent.setup();
		setReauthenticator(() => reauth.request());
		const fetchFn = mockFetch(
			problem(403, 'reauth_required'),
			methods(ReauthMethod.PASSWORD, ReauthMethod.EMAIL_CODE),
			problem(422, 'validation_failed', {
				errors: [{ field: 'password', code: 'invalid', message: 'is not right' }]
			}),
			noContent(),
			reply(RecoveryCodesSchema, { codes: ['aaaaa-bbbbb'] })
		);
		vi.stubGlobal('fetch', fetchFn);
		render(ReauthDialog);

		const regenerate = api.me.regenerateRecoveryCodes();
		expect(await screen.findByRole('dialog', { name: t('reauth-title') })).toBeInTheDocument();

		const password = await screen.findByLabelText(t('reauth-password-label'));
		await user.type(password, 'wrong');
		await user.click(screen.getByRole('button', { name: t('reauth-confirm') }));
		await vi.waitFor(() => expect(password).toHaveAccessibleDescription('is not right'));

		await user.clear(password);
		await user.type(password, 'secret');
		await user.click(screen.getByRole('button', { name: t('reauth-confirm') }));

		await expect(regenerate).resolves.toEqual(
			create(RecoveryCodesSchema, { codes: ['aaaaa-bbbbb'] })
		);
		expect(sent(fetchFn, ReauthenticateRequestSchema, 3)).toEqual(
			reauthenticated(ReauthMethod.PASSWORD, 'secret')
		);
		expect(callOf(fetchFn, 4)[0]).toBe('/api/v1/me/mfa/recovery-codes');
		await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('offers the other methods, e.g. an emailed code', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(
			methods(ReauthMethod.TOTP, ReauthMethod.EMAIL_CODE, ReauthMethod.UNSPECIFIED),
			new Response(null, { status: 202 }),
			noContent()
		);
		vi.stubGlobal('fetch', fetchFn);
		render(ReauthDialog);

		const confirmed = reauth.request();
		expect(await screen.findByLabelText(t('reauth-code-app'))).toBeInTheDocument();
		expect(screen.getAllByRole('button', { name: /instead$/ })).toHaveLength(1);
		await user.click(screen.getByRole('button', { name: t('reauth-email-instead') }));
		await user.click(screen.getByRole('button', { name: t('reauth-email-me') }));
		await user.type(await screen.findByLabelText(t('reauth-code-email')), '123456');
		await user.click(screen.getByRole('button', { name: t('reauth-confirm') }));

		await expect(confirmed).resolves.toBe(true);
		expect(callOf(fetchFn, 1)[0]).toBe('/api/v1/me/reauthenticate/email-code');
		expect(sent(fetchFn, ReauthenticateRequestSchema, 2)).toEqual(
			reauthenticated(ReauthMethod.EMAIL_CODE, '123456')
		);
	});

	it('shows every field error about the secret on its input', async () => {
		const user = userEvent.setup();
		const invalid = (field: string, message: string) =>
			problem(422, 'validation_failed', { errors: [{ field, code: 'invalid', message }] });
		vi.stubGlobal(
			'fetch',
			mockFetch(
				methods(ReauthMethod.TOTP),
				invalid('code', 'is not right'),
				invalid('secret', 'is too long'),
				invalid('method', 'is not offered')
			)
		);
		render(ReauthDialog);
		void reauth.request();

		const code = await screen.findByLabelText(t('reauth-code-app'));
		await user.type(code, '123456');
		for (const message of ['is not right', 'is too long', 'is not offered']) {
			await user.click(screen.getByRole('button', { name: t('reauth-confirm') }));
			await vi.waitFor(() => expect(code).toHaveAccessibleDescription(message));
		}
		expect(screen.getAllByRole('alert')).toHaveLength(1);

		await user.type(code, '1');
		expect(code).not.toHaveAccessibleDescription('is not offered');
	});

	it('cancels, and the caller gets the error', async () => {
		const user = userEvent.setup();
		setReauthenticator(() => reauth.request());
		vi.stubGlobal('fetch', mockFetch(problem(403, 'reauth_required'), methods(ReauthMethod.TOTP)));
		render(ReauthDialog);

		const change = api.me.changeEmail({ email: 'new@example.com' }).catch((e: unknown) => e);
		await screen.findByLabelText(t('reauth-code-app'));
		await user.keyboard('{Escape}');

		await expect(change).resolves.toMatchObject({ code: 'reauth_required' });
	});

	it('says when the methods cannot be loaded', async () => {
		vi.stubGlobal('fetch', mockFetch(problem(503, 'unavailable')));
		render(ReauthDialog);
		void reauth.request();
		expect(await screen.findByRole('alert')).toHaveTextContent('problem unavailable');
	});
});

describe('cancelling an email change', () => {
	it('cancels only on a click, then points at a new password', async () => {
		const user = userEvent.setup();
		visit('/cancel-email-change#token=abc');
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		render(CancelEmailChange);

		const button = await screen.findByRole('button', { name: t('cancel-submit') });
		expect(page.url.hash).toBe('');
		expect(fetchFn).not.toHaveBeenCalled();

		await user.click(button);
		expect(
			await screen.findByRole('heading', { name: t('cancel-done-title') })
		).toBeInTheDocument();
		expect(sent(fetchFn, CancelEmailChangeRequestSchema)).toEqual(
			create(CancelEmailChangeRequestSchema, { token: 'abc' })
		);
		expect(screen.getByText(t('cancel-done-description'))).toBeInTheDocument();
		expect(screen.getByRole('link', { name: t('cancel-new-password') })).toHaveAttribute(
			'href',
			'/forgot-password'
		);
		expect(navigation.invalidate).toHaveBeenCalledWith('app:session');
	});

	it('explains a used link or a taken address', async () => {
		const user = userEvent.setup();
		visit('/cancel-email-change#token=abc');
		vi.stubGlobal('fetch', mockFetch(problem(409, 'email_taken')));
		render(CancelEmailChange);

		await user.click(await screen.findByRole('button', { name: t('cancel-submit') }));
		expect(
			await screen.findByRole('heading', { name: t('cancel-failed-title') })
		).toBeInTheDocument();
		expect(screen.getByText('problem email_taken')).toBeInTheDocument();
	});

	it('explains an incomplete link', async () => {
		render(CancelEmailChange);
		expect(await screen.findByText(t('cancel-incomplete'))).toBeInTheDocument();
	});
});
