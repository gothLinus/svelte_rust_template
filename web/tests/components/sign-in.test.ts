import { render, screen } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { describe, expect, it, vi } from 'vitest';
import Login from '../../src/routes/(public)/(guest)/login/+page.svelte';
import Mfa from '../../src/routes/(public)/(guest)/login/mfa/+page.svelte';
import MagicLink from '../../src/routes/(public)/magic-link/+page.svelte';
import { create } from '@bufbuild/protobuf';
import { t } from '$lib/i18n';
import {
	LoginRequestSchema,
	MagicLinkRequestSchema,
	MeSchema,
	MfaChallengeSchema,
	MfaMethod
} from '$lib/types/api';
import { me, mockFetch, problem, reply, rootData, sent } from '../helpers';
import { navigation, page, visit } from './fake-app.svelte';

const data = rootData(null, { providers: [{ id: 'github', name: 'GitHub' }] });

describe('login page', () => {
	it('signs in and goes where the user was headed', async () => {
		const user = userEvent.setup();
		visit('/login?redirectTo=%2Fadmin%2Fusers');
		const fetchFn = mockFetch(reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);
		render(Login, { data });

		await user.type(screen.getByLabelText(t('login-identifier')), 'alice');
		await user.type(screen.getByLabelText(t('login-password')), 'secret');
		await user.click(screen.getByRole('button', { name: t('login-submit') }));

		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/admin/users', { invalidateAll: true })
		);
		expect(sent(fetchFn, LoginRequestSchema)).toEqual(
			create(LoginRequestSchema, { identifier: 'alice', password: 'secret' })
		);
		expect(screen.getByRole('link', { name: t('login-create-account') })).toHaveAttribute(
			'href',
			'/register?redirectTo=%2Fadmin%2Fusers'
		);
		expect(screen.getByRole('link', { name: /GitHub/ }).getAttribute('href')).toContain(
			'redirectTo=%2Fadmin%2Fusers'
		);
	});

	it('continues with the second step', async () => {
		const user = userEvent.setup();
		vi.stubGlobal(
			'fetch',
			mockFetch(reply(MfaChallengeSchema, { methods: [MfaMethod.TOTP] }, 202))
		);
		render(Login, { data });

		await user.type(screen.getByLabelText(t('login-identifier')), 'alice');
		await user.type(screen.getByLabelText(t('login-password')), 'secret');
		await user.click(screen.getByRole('button', { name: t('login-submit') }));

		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/login/mfa?redirectTo=%2Fdashboard')
		);
	});

	it('shows wrong credentials and required fields', async () => {
		const user = userEvent.setup();
		vi.stubGlobal('fetch', mockFetch(problem(401, 'invalid_credentials')));
		render(Login, { data });

		await user.click(screen.getByRole('button', { name: t('login-submit') }));
		expect(screen.getByLabelText(t('login-identifier'))).toHaveFocus();
		expect(screen.getByLabelText(t('login-password'))).toHaveAccessibleDescription(
			t('validation-required')
		);

		await user.type(screen.getByLabelText(t('login-identifier')), 'alice');
		await user.type(screen.getByLabelText(t('login-password')), 'wrong');
		await user.click(screen.getByRole('button', { name: t('login-submit') }));
		expect(await screen.findByRole('alert')).toHaveTextContent('problem invalid_credentials');
		expect(navigation.goto).not.toHaveBeenCalled();
	});

	it('explains a failed social sign-in', () => {
		visit('/login?error=oauth_cancelled');
		render(Login, { data });
		expect(screen.getByRole('alert')).toHaveTextContent(t('signin-oauth-error-oauth-cancelled'));
	});

	it('toggles the password with a pressed state, not a changing name', async () => {
		const user = userEvent.setup();
		render(Login, { data });

		const toggle = screen.getByRole('button', { name: t('signin-show-password') });
		expect(toggle).toHaveAttribute('aria-pressed', 'false');
		await user.click(toggle);
		expect(toggle).toHaveAttribute('aria-pressed', 'true');
		expect(toggle).toHaveAccessibleName(t('signin-show-password'));
		expect(screen.getByLabelText(t('login-password'))).toHaveAttribute('type', 'text');
	});
});

describe('second step', () => {
	function mfa(...methods: MfaMethod[]) {
		vi.stubGlobal(
			'fetch',
			mockFetch(reply(MfaChallengeSchema, { methods }), reply(MeSchema, me()))
		);
		render(Mfa);
	}

	it('asks for the app code, and offers a recovery code', async () => {
		const user = userEvent.setup();
		visit('/login/mfa?redirectTo=%2Fnotes');
		mfa(MfaMethod.TOTP, MfaMethod.RECOVERY_CODE);

		const code = await screen.findByLabelText(t('mfa-label-totp'));
		expect(code).toHaveAttribute('autocomplete', 'one-time-code');

		await user.click(screen.getByRole('button', { name: t('mfa-use-recovery') }));
		expect(screen.getByLabelText(t('mfa-label-recovery'))).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: t('mfa-use-totp') }));

		await user.type(screen.getByLabelText(t('mfa-label-totp')), '123456');
		await user.click(screen.getByRole('button', { name: t('mfa-submit') }));
		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/notes', { invalidateAll: true })
		);
	});

	it('offers only the passkey when no recovery codes are left', async () => {
		mfa(MfaMethod.PASSKEY);

		expect(await screen.findByRole('button', { name: t('mfa-passkey') })).toBeInTheDocument();
		expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
		expect(screen.queryByRole('button', { name: /recovery code/ })).not.toBeInTheDocument();
		expect(screen.getByText(t('mfa-description-passkey'))).toBeInTheDocument();
	});

	it('starts with a recovery code when there is no authenticator app', async () => {
		const user = userEvent.setup();
		mfa(MfaMethod.PASSKEY, MfaMethod.RECOVERY_CODE);

		expect(await screen.findByLabelText(t('mfa-label-recovery'))).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: t('mfa-back') }));
		expect(screen.queryByRole('textbox')).not.toBeInTheDocument();
	});

	it('says when the attempt expired', async () => {
		vi.stubGlobal('fetch', mockFetch(problem(401, 'unauthenticated')));
		render(Mfa);
		expect(await screen.findByText(t('mfa-expired'))).toBeInTheDocument();
	});
});

describe('magic link', () => {
	it('signs in only when the user clicks Continue', async () => {
		const user = userEvent.setup();
		visit('/magic-link#token=abc');
		const fetchFn = mockFetch(reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);
		render(MagicLink);

		const button = await screen.findByRole('button', { name: t('magic-continue') });
		expect(page.url.hash).toBe('');
		expect(fetchFn).not.toHaveBeenCalled();

		await user.click(button);
		expect(sent(fetchFn, MagicLinkRequestSchema)).toEqual(
			create(MagicLinkRequestSchema, { token: 'abc' })
		);
		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/dashboard', { invalidateAll: true })
		);
	});

	it('explains an incomplete or used link', async () => {
		const user = userEvent.setup();
		render(MagicLink);
		expect(
			await screen.findByText((text) => text.includes(t('magic-incomplete')))
		).toBeInTheDocument();

		visit('/magic-link#token=used');
		vi.stubGlobal('fetch', mockFetch(problem(400, 'invalid_token')));
		render(MagicLink);
		await user.click(await screen.findByRole('button', { name: t('magic-continue') }));
		// The server's lowercase detail is made a sentence before the advice that follows it.
		expect(
			await screen.findByText((text) => text.startsWith('Problem invalid_token. '))
		).toBeInTheDocument();
	});
});
