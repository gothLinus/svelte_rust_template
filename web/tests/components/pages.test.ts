import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { createRawSnippet, tick } from 'svelte';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { session } from '$lib/auth';
import { i18n, t } from '$lib/i18n';
import { create } from '@bufbuild/protobuf';
import {
	MeSchema,
	Permission,
	ResetPasswordRequestSchema,
	RoleSchema,
	TextChannel,
	type User,
	UserPageSchema,
	UserSchema,
	VerifyPhoneCodeRequestSchema
} from '$lib/types/api';
import UserMenu from '$lib/components/user-menu.svelte';
import EmailVerificationBanner from '$lib/components/email-verification-banner.svelte';
import AppLayout from '../../src/routes/(app)/+layout.svelte';
import AppError from '../../src/routes/(app)/+error.svelte';
import UsersPage from '../../src/routes/(app)/admin/users/+page.svelte';
import Dashboard from '../../src/routes/(app)/dashboard/+page.svelte';
import RootError from '../../src/routes/+error.svelte';
import RootLayout from '../../src/routes/+layout.svelte';
import Landing from '../../src/routes/+page.svelte';
import PublicLayout from '../../src/routes/(public)/+layout.svelte';
import EmailLogin from '../../src/routes/(public)/(guest)/login/email/+page.svelte';
import PhoneLogin from '../../src/routes/(public)/(guest)/login/phone/+page.svelte';
import ConfirmEmail from '../../src/routes/(public)/confirm-email/+page.svelte';
import ResetPassword from '../../src/routes/(public)/reset-password/+page.svelte';
import VerifyEmail from '../../src/routes/(public)/verify-email/+page.svelte';
import {
	OTHER_ID,
	callOf,
	makeUser,
	me,
	mockFetch,
	noContent,
	note,
	problem,
	reply,
	rootData,
	sent
} from '../helpers';
import { navigation, page, visit } from './fake-app.svelte';

const children = createRawSnippet(() => ({ render: () => '<p>Page content</p>' }));

beforeEach(() => session.connect(() => null));

describe('root layout', () => {
	it('offers a retry when the session could not be checked', () => {
		render(RootLayout, { data: { ...rootData(), sessionError: 'The server is down.' }, children });
		expect(screen.getByText('The server is down.')).toBeInTheDocument();
		expect(screen.getByText('Page content')).toBeInTheDocument();
	});

	it('connects the session to its data', () => {
		render(RootLayout, { data: rootData(me()), children });
		expect(session.isAuthenticated).toBe(true);
		expect(screen.queryByRole('alert')).not.toBeInTheDocument();
	});
});

describe('error pages', () => {
	it('offers a retry for server errors', async () => {
		const user = userEvent.setup();
		page.status = 503;
		page.error = { message: 'The server is down.' };
		render(RootError);

		expect(screen.getByRole('heading', { name: 'The server is down.' })).toBeInTheDocument();
		// Tests run in development mode, so the hint shows here; `dev` hides it in builds.
		expect(screen.getByText(/just dev/)).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: t('app-error-retry') }));
		expect(navigation.invalidateAll).toHaveBeenCalledOnce();
	});

	it('links back to the dashboard inside the app', () => {
		page.status = 404;
		page.error = { message: 'This page does not exist.' };
		render(AppError);
		expect(screen.getByRole('link', { name: t('app-error-back-to-dashboard') })).toHaveAttribute(
			'href',
			'/dashboard'
		);
	});
});

describe('app layout', () => {
	it('has a skip link to the main landmark and a main navigation', async () => {
		const user = userEvent.setup();
		visit('/notes');
		const unverified = me([Permission.NOTES_READ]);
		unverified.user.emailVerified = false;
		render(AppLayout, { data: rootData(unverified), children });

		const skip = screen.getByRole('link', { name: t('app-skip-to-content') });
		await user.click(skip);
		expect(screen.getByRole('main')).toHaveFocus();
		expect(screen.getAllByRole('navigation', { name: t('nav-main') }).length).toBeGreaterThan(0);
		expect(screen.getByText(t('email-verification-title'))).toBeInTheDocument();
	});

	it('remembers whether the sidebar is open', async () => {
		render(AppLayout, { data: rootData(me()), children });
		await fireEvent.click(screen.getAllByRole('button', { name: t('nav-toggle-sidebar') })[0]!);
		expect(localStorage.getItem('sidebar-open')).toBe('false');
	});
});

describe('language', () => {
	// A made-up language, so the test does not depend on which real ones the catalog has.
	const TEST_LANGUAGE = 'tlh';

	afterEach(() => i18n.use('en'));

	it('renders a page again in the new language, falling back to English for the rest', async () => {
		i18n.register(TEST_LANGUAGE, [
			'landing-headline = Konten, Sitzungen und Rechte, vom ersten Tag an.\nlanding-sign-in = Anmelden'
		]);
		render(Landing, { data: rootData() });
		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(t('landing-headline'));
		expect(screen.getAllByRole('link', { name: 'Sign in' })).toHaveLength(2);

		await i18n.use(TEST_LANGUAGE);
		await tick();

		expect(screen.getByRole('heading', { level: 1 })).toHaveTextContent(
			'Konten, Sitzungen und Rechte'
		);
		expect(screen.getAllByRole('link', { name: 'Anmelden' })).toHaveLength(2);
		expect(screen.getByRole('link', { name: t('landing-create-account') })).toBeInTheDocument();
		expect(document.documentElement.lang).toBe(TEST_LANGUAGE);
	});
});

describe('landing and dashboard', () => {
	it('invites visitors in and sends users to their dashboard', () => {
		render(Landing, { data: rootData() });
		expect(
			screen.getAllByRole('link', { name: new RegExp(t('landing-sign-in')) })[0]
		).toHaveAttribute('href', '/login');

		const { container } = render(PublicLayout, { children });
		expect(within(container).getByRole('main')).toHaveTextContent('Page content');
	});

	it('summarises the account', () => {
		render(Dashboard, {
			data: { ...rootData(me()), sessions: [], recentNotes: [note()] }
		});
		expect(
			screen.getByRole('heading', { level: 1, name: t('dashboard-title', { name: 'alice' }) })
		).toBeTruthy();
		expect(
			screen.getByRole('heading', { level: 2, name: t('dashboard-recent-title') })
		).toBeTruthy();
		expect(screen.getByText('Groceries')).toBeInTheDocument();
	});
});

describe('admin users', () => {
	function user(id: string, username: string, disabled = false): User {
		return makeUser({ id, username, disabled });
	}

	function renderUsers() {
		return render(UsersPage, {
			data: {
				...rootData(me([Permission.USERS_READ, Permission.USERS_MANAGE])),
				users: create(UserPageSchema, {
					items: [user(me().user.id, 'alice'), user(OTHER_ID, 'bob')],
					nextCursor: 'c1'
				}),
				roles: [
					create(RoleSchema, { name: 'user', description: 'Everyone' }),
					create(RoleSchema, { name: 'admin', description: 'Everything' })
				],
				search: '',
				after: null
			}
		});
	}

	async function openMenu(name: string) {
		await fireEvent.pointerDown(screen.getByRole('button', { name }), {
			button: 0,
			pointerType: 'mouse'
		});
	}

	it('asks before disabling an account', async () => {
		const fetchFn = mockFetch(reply(UserSchema, user(OTHER_ID, 'bob', true)));
		vi.stubGlobal('fetch', fetchFn);
		renderUsers();

		await openMenu(t('admin-manage-label', { name: 'bob' }));
		await fireEvent.click(await screen.findByRole('menuitem', { name: t('admin-disable') }));
		const dialog = await screen.findByRole('alertdialog', {
			name: t('admin-disable-title', { name: 'bob' })
		});
		expect(fetchFn).not.toHaveBeenCalled();

		await fireEvent.click(within(dialog).getByRole('button', { name: t('admin-disable-confirm') }));
		await vi.waitFor(() =>
			expect(callOf(fetchFn)[0]).toBe(`/api/v1/admin/users/${OTHER_ID}/disable`)
		);
	});

	it('grants roles, searches and pages', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(reply(UserSchema, me().user));
		vi.stubGlobal('fetch', fetchFn);
		visit('/admin/users');
		renderUsers();

		await openMenu(t('admin-manage-label', { name: 'bob' }));
		await fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: 'admin' }));
		await vi.waitFor(() =>
			expect(callOf(fetchFn)[0]).toBe(`/api/v1/admin/users/${OTHER_ID}/roles/admin`)
		);

		// The closed menu leaves its page lock behind in jsdom.
		document.body.removeAttribute('style');
		const searchbox = screen.getByRole('searchbox', { name: t('admin-search-label') });
		// Shorter terms would match nearly everyone; the server refuses them.
		await user.type(searchbox, 'bo{Enter}');
		expect(navigation.goto).not.toHaveBeenCalledWith('/admin/users?search=bo', expect.anything());
		await user.type(searchbox, 'b{Enter}');
		expect(navigation.goto).toHaveBeenLastCalledWith('/admin/users?search=bob', expect.anything());

		await user.click(screen.getByRole('button', { name: t('pagination-next') }));
		expect(navigation.goto).toHaveBeenLastCalledWith('/admin/users?after=c1', expect.anything());
	});

	it('shows failures', async () => {
		vi.stubGlobal('fetch', mockFetch(problem(409, 'last_admin')));
		renderUsers();
		await openMenu(t('admin-manage-label', { name: 'bob' }));
		await fireEvent.click(await screen.findByRole('menuitemcheckbox', { name: 'admin' }));
		await vi.waitFor(() => expect(navigation.invalidate).not.toHaveBeenCalled());
	});
});

describe('passwordless sign-in', () => {
	it('signs in with an emailed code', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(new Response(null, { status: 202 }), reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);
		render(EmailLogin);

		await user.type(screen.getByLabelText(t('login-email-label')), 'alice@example.com');
		await user.click(screen.getByRole('button', { name: t('login-email-send') }));
		await user.type(await screen.findByLabelText(t('login-email-code')), '123 456');
		await user.click(screen.getByRole('button', { name: t('login-email-submit') }));

		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/dashboard', { invalidateAll: true })
		);
		await user.click(screen.getByRole('button', { name: t('login-email-another') }));
		expect(screen.getByLabelText(t('login-email-label'))).toBeInTheDocument();
	});

	it('signs in with a texted code', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(new Response(null, { status: 202 }), reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);
		render(PhoneLogin, { data: rootData(null, { textChannels: [TextChannel.SMS] }) });

		await user.type(screen.getByLabelText(t('phone-label')), '+49 170 1234567');
		await user.click(screen.getByRole('button', { name: t('phone-send') }));
		await user.type(await screen.findByLabelText(t('login-email-code')), '123456');
		await user.click(screen.getByRole('button', { name: t('phone-submit') }));

		await vi.waitFor(() => expect(navigation.goto).toHaveBeenCalled());
		expect(sent(fetchFn, VerifyPhoneCodeRequestSchema, 1)).toEqual(
			create(VerifyPhoneCodeRequestSchema, { phone: '+49 170 1234567', code: '123456' })
		);
		await user.click(screen.getByRole('button', { name: t('phone-another') }));
	});

	it('says when texting is not set up', () => {
		render(PhoneLogin, {
			data: rootData()
		});
		expect(screen.getByText(t('phone-unavailable'))).toBeInTheDocument();
	});
});

describe('emailed links', () => {
	it('resets the password from the link', async () => {
		const user = userEvent.setup();
		visit('/reset-password#token=abc');
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		render(ResetPassword);

		await user.type(screen.getByLabelText(t('reset-password')), 'correct horse');
		await user.type(screen.getByLabelText(t('reset-confirmation')), 'correct horse');
		await user.click(screen.getByRole('button', { name: t('reset-submit') }));

		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/login', { invalidateAll: true })
		);
		expect(sent(fetchFn, ResetPasswordRequestSchema)).toEqual(
			create(ResetPasswordRequestSchema, { token: 'abc', password: 'correct horse' })
		);
	});

	it('explains an incomplete reset link', async () => {
		render(ResetPassword);
		expect(await screen.findByRole('alert')).toHaveTextContent(t('reset-incomplete'));
	});

	it('verifies the address, and tells the other tabs', async () => {
		// The tab with the "verify your email" banner, still open next to this one.
		const otherTab = new BroadcastChannel('app:sync');
		const heard = new Promise((resolve) =>
			otherTab.addEventListener('message', (event) => resolve(event.data), { once: true })
		);
		visit('/verify-email#token=abc');
		vi.stubGlobal('fetch', mockFetch(noContent()));
		render(VerifyEmail);
		expect(
			await screen.findByRole('heading', { name: t('verify-done-title') })
		).toBeInTheDocument();
		expect(navigation.invalidate).toHaveBeenCalledWith('app:session');
		expect(await heard).toBe('app:session');
		otherTab.close();
	});

	it('confirms a new address, or explains why not', async () => {
		visit('/confirm-email#token=abc');
		vi.stubGlobal('fetch', mockFetch(problem(400, 'invalid_token')));
		render(ConfirmEmail);
		expect(
			await screen.findByRole('heading', { name: t('confirm-failed-title') })
		).toBeInTheDocument();

		visit('/confirm-email#token=def');
		session.connect(() => me());
		vi.stubGlobal('fetch', mockFetch(noContent()));
		render(ConfirmEmail);
		expect(
			await screen.findByRole('heading', { name: t('confirm-done-title') })
		).toBeInTheDocument();
		expect(navigation.invalidate).toHaveBeenCalledWith('app:session');
	});
});

describe('account', () => {
	it('signs out from the account menu', async () => {
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		render(UserMenu, { user: me().user });

		await fireEvent.pointerDown(screen.getByRole('button', { name: t('nav-account-menu') }), {
			button: 0,
			pointerType: 'mouse'
		});
		await fireEvent.click(await screen.findByRole('menuitem', { name: t('nav-sign-out') }));
		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/login', { invalidateAll: true })
		);
		expect(callOf(fetchFn)[0]).toBe('/api/v1/auth/logout');
	});

	it('resends the verification link', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(new Response(null, { status: 202 }), problem(429, 'rate_limited'));
		vi.stubGlobal('fetch', fetchFn);
		render(EmailVerificationBanner, { email: 'alice@example.com' });

		await user.click(screen.getByRole('button', { name: t('email-verification-resend') }));
		await user.click(screen.getByRole('button', { name: t('email-verification-resend') }));
		await vi.waitFor(() => expect(fetchFn).toHaveBeenCalledTimes(2));
	});

	it('explains a failed verification', async () => {
		visit('/verify-email#token=used');
		vi.stubGlobal('fetch', mockFetch(problem(400, 'invalid_token')));
		render(VerifyEmail);
		expect(
			await screen.findByRole('heading', { name: t('verify-failed-title') })
		).toBeInTheDocument();
		expect(screen.getByRole('link', { name: t('verify-sign-in') })).toBeInTheDocument();
	});
});
