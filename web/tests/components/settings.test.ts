import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { toast } from 'svelte-sonner';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import { reauth, session } from '$lib/auth';
import { formatDate, formatRelative } from '$lib/helpers/format';
import { t } from '$lib/i18n';
import { type MessageInitShape, create } from '@bufbuild/protobuf';
import {
	AddPhoneRequestSchema,
	type AuditEventPage,
	AuditEventPageSchema,
	CodeRequestSchema,
	MeSchema,
	PasskeySchema,
	RecoveryCodesSchema,
	RenamePasskeyRequestSchema,
	SecondFactorAddedSchema,
	type SecurityOverview,
	SecurityOverviewSchema,
	type Session,
	SessionSchema,
	TextChannel,
	TotpSetupSchema
} from '$lib/types/api';
import ProfilePage from '../../src/routes/(app)/settings/profile/+page.svelte';
import SecurityPage from '../../src/routes/(app)/settings/security/+page.svelte';
import {
	callOf,
	me,
	mockFetch,
	noContent,
	problem,
	reply,
	rootData,
	sent,
	ts,
	useEnglish,
	useGerman
} from '../helpers';
import { navigation, page, visit } from './fake-app.svelte';

const FIREFOX = 'Mozilla/5.0 (Macintosh; Intel Mac OS X 14_0) Gecko/20100101 Firefox/130.0';
const IPHONE = 'Mozilla/5.0 (iPhone; CPU iPhone OS 17_0 like Mac OS X) Safari/604.1';

const SAFARI_IOS = t('session-device', { browser: 'Safari', system: 'iOS' });
const FIREFOX_MACOS = t('session-device', { browser: 'Firefox', system: 'macOS' });

const signOutOf = (device: string) => (name: string) =>
	name.startsWith(t('security-sessions-sign-out-label', { device, active: '' }).trimEnd());

function sessionRow(id: string, userAgent: string, current = false): Session {
	return create(SessionSchema, {
		id,
		current,
		ip: '192.0.2.1',
		userAgent,
		createdAt: ts('2026-01-01T00:00:00Z'),
		lastSeenAt: ts('2026-01-02T00:00:00Z'),
		expiresAt: ts('2026-02-01T00:00:00Z')
	});
}

function security(
	overrides: MessageInitShape<typeof SecurityOverviewSchema> = {}
): SecurityOverview {
	return create(SecurityOverviewSchema, {
		hasPassword: true,
		mfaEnabled: true,
		totpEnabled: true,
		recoveryCodesRemaining: 8,
		passkeys: [
			{ id: 'p1', name: 'MacBook', createdAt: ts('2026-01-01T00:00:00Z') },
			{
				id: 'p2',
				name: 'YubiKey',
				createdAt: ts('2026-01-01T00:00:00Z'),
				lastUsedAt: ts('2026-01-02T00:00:00Z')
			}
		],
		linkedAccounts: [{ provider: 'github', providerName: 'GitHub', email: 'a@gh.example' }],
		...overrides
	});
}

const providers = [
	{ id: 'github', name: 'GitHub' },
	{ id: 'google', name: 'Google' }
];

function activity(overrides: MessageInitShape<typeof AuditEventPageSchema> = {}): AuditEventPage {
	return create(AuditEventPageSchema, {
		items: [
			{
				id: 'e3',
				action: 'role_granted',
				detail: 'admin',
				byOther: true,
				occurredAt: ts('2026-01-03T00:00:00Z')
			},
			{
				id: 'e2',
				action: 'sign_in_failed',
				detail: 'password',
				ip: '192.0.2.9',
				userAgent: FIREFOX,
				occurredAt: ts('2026-01-02T00:00:00Z')
			},
			{
				id: 'e1',
				action: 'signed_in',
				detail: 'provider:github',
				occurredAt: ts('2026-01-01T00:00:00Z')
			}
		],
		...overrides
	});
}

function renderSecurity(overview = security(), events = activity()) {
	return render(SecurityPage, {
		data: {
			...rootData(me(), { providers }),
			security: overview,
			sessions: [sessionRow('s1', FIREFOX, true), sessionRow('s2', IPHONE)],
			activity: events
		}
	});
}

beforeEach(() => {
	session.connect(() => me());
	visit('/settings/security');
});

describe('security page', () => {
	it('heads each card and names every row control uniquely', () => {
		renderSecurity();

		for (const heading of [
			t('security-password-title'),
			`${t('security-two-step-title')} ${t('security-two-step-on')}`,
			t('security-passkeys-title'),
			t('security-linked-title'),
			t('security-sessions-title'),
			t('security-activity-title')
		]) {
			expect(screen.getByRole('heading', { level: 2, name: heading })).toBeInTheDocument();
		}
		expect(
			screen.getByRole('button', { name: t('security-passkeys-remove-label', { name: 'MacBook' }) })
		).toBeInTheDocument();
		expect(
			screen.getByRole('button', { name: t('security-passkeys-rename-label', { name: 'YubiKey' }) })
		).toBeInTheDocument();
		expect(
			screen.getByRole('button', {
				name: t('security-linked-unlink-label', { provider: 'GitHub' })
			})
		).toBeInTheDocument();
		expect(
			screen.getByRole('link', { name: t('security-linked-link-label', { provider: 'Google' }) })
		).toBeInTheDocument();
		expect(screen.getByRole('button', { name: signOutOf(SAFARI_IOS) })).toBeInTheDocument();
		expect(
			screen.queryByRole('button', { name: signOutOf(FIREFOX_MACOS) })
		).not.toBeInTheDocument();
	});

	it('reports a linked account and tidies the address', () => {
		visit('/settings/security?linked=github');
		renderSecurity();
		expect(navigation.replaceState).toHaveBeenCalledWith('/settings/security', page.state);
	});

	it('confirms it is the user when linking needs that, then links again', async () => {
		renderSecurity();
		const link = screen.getByRole('link', {
			name: t('security-linked-link-label', { provider: 'Google' })
		});
		link.addEventListener('click', (event) => event.preventDefault());
		await fireEvent.click(link);
		expect(sessionStorage.getItem('linking-provider')).toBe('google');

		visit('/settings/security?error=reauth_required');
		renderSecurity();
		expect(reauth.open).toBe(true);
		expect(sessionStorage.getItem('linking-provider')).toBeNull();
		reauth.finish(false);
	});

	it('asks before removing a passkey', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		renderSecurity();

		await user.click(
			screen.getByRole('button', { name: t('security-passkeys-remove-label', { name: 'MacBook' }) })
		);
		const dialog = await screen.findByRole('alertdialog', {
			name: t('security-passkeys-remove-title', { name: 'MacBook' })
		});
		expect(dialog).toHaveTextContent(t('security-passkeys-remove-description-two-step'));
		expect(fetchFn).not.toHaveBeenCalled();

		await user.click(within(dialog).getByRole('button', { name: t('security-passkeys-remove') }));
		await vi.waitFor(() => expect(callOf(fetchFn)[0]).toBe('/api/v1/me/passkeys/p1'));
		expect(navigation.invalidate).toHaveBeenCalledWith('app:security');
	});

	it('renames a passkey, within the length limit', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(reply(PasskeySchema, { id: 'p1', name: 'Work laptop' }));
		vi.stubGlobal('fetch', fetchFn);
		renderSecurity();

		await user.click(screen.getByRole('button', { name: 'Rename MacBook' }));
		const name = await screen.findByLabelText(t('security-passkeys-name'));
		expect(name).toHaveValue('MacBook');
		await user.clear(name);
		await user.click(screen.getByRole('button', { name: t('security-passkeys-save') }));
		expect(name).toHaveAccessibleDescription(t('validation-required'));

		await user.type(name, 'Work laptop');
		await user.click(screen.getByRole('button', { name: t('security-passkeys-save') }));
		await vi.waitFor(() =>
			expect(sent(fetchFn, RenamePasskeyRequestSchema)).toEqual(
				create(RenamePasskeyRequestSchema, { name: 'Work laptop' })
			)
		);
	});

	it('asks before making new recovery codes, then shows them', async () => {
		const user = userEvent.setup();
		vi.stubGlobal('fetch', mockFetch(reply(RecoveryCodesSchema, { codes: ['aaaaa-bbbbb'] })));
		renderSecurity();

		await user.click(screen.getByRole('button', { name: t('security-recovery-new-label') }));
		const dialog = await screen.findByRole('alertdialog', {
			name: t('security-recovery-new-title')
		});
		await user.click(
			within(dialog).getByRole('button', { name: t('security-recovery-new-confirm') })
		);

		expect(
			await screen.findByRole('dialog', { name: t('security-recovery-dialog-title') })
		).toHaveTextContent('aaaaa-bbbbb');
	});

	it('removes the authenticator app with a current code', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		renderSecurity();

		await user.click(screen.getByRole('button', { name: t('security-totp-remove-label') }));
		const code = await screen.findByLabelText(t('security-totp-code'));
		await user.type(code, '123456');
		await user.click(screen.getByRole('button', { name: t('security-totp-remove') }));

		await vi.waitFor(() => expect(callOf(fetchFn)[1].method).toBe('DELETE'));
		expect(sent(fetchFn, CodeRequestSchema)).toEqual(create(CodeRequestSchema, { code: '123456' }));
	});

	it('sets up an authenticator app with a labelled, copyable key', async () => {
		const user = userEvent.setup();
		const writeText = vi.spyOn(navigator.clipboard, 'writeText');
		vi.stubGlobal(
			'fetch',
			mockFetch(
				reply(TotpSetupSchema, { secret: 'ABCDEFGHIJKLMNOP', uri: 'otpauth://totp/Acme:alice' }),
				reply(SecondFactorAddedSchema, { recoveryCodes: { codes: ['aaaaa-bbbbb'] } })
			)
		);
		renderSecurity(security({ totpEnabled: false, mfaEnabled: false, passkeys: [] }));

		await user.click(screen.getByRole('button', { name: t('security-totp-setup-label') }));
		expect(
			await screen.findByRole('img', { name: t('security-totp-qr-label') })
		).toBeInTheDocument();
		expect(screen.getByLabelText(t('security-totp-key'))).toHaveTextContent('ABCD EFGH IJKL MNOP');
		await user.click(screen.getByRole('button', { name: t('security-totp-copy-key') }));
		expect(writeText).toHaveBeenCalledWith('ABCDEFGHIJKLMNOP');

		await user.type(screen.getByLabelText(t('security-totp-code')), '123456');
		await user.click(screen.getByRole('button', { name: t('security-totp-turn-on') }));
		expect(
			await screen.findByRole('dialog', { name: t('security-recovery-dialog-title') })
		).toHaveTextContent('aaaaa-bbbbb');
	});

	it('signs out another device, and asks before signing out everywhere', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		renderSecurity();

		await user.click(screen.getByRole('button', { name: signOutOf(SAFARI_IOS) }));
		await vi.waitFor(() => expect(callOf(fetchFn)[0]).toBe('/api/v1/me/sessions/s2'));

		await user.click(screen.getByRole('button', { name: t('security-sessions-everywhere') }));
		const dialog = await screen.findByRole('alertdialog', {
			name: t('security-sessions-everywhere-title')
		});
		await user.click(
			within(dialog).getByRole('button', { name: t('security-sessions-everywhere') })
		);
		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/login', { invalidateAll: true })
		);
	});

	it('unlinks a provider and asks for a password link', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		renderSecurity();

		await user.click(
			screen.getByRole('button', {
				name: t('security-linked-unlink-label', { provider: 'GitHub' })
			})
		);
		await vi.waitFor(() => expect(callOf(fetchFn)[0]).toBe('/api/v1/me/linked-accounts/github'));

		await user.click(screen.getByRole('button', { name: t('security-password-change') }));
		await vi.waitFor(() => expect(callOf(fetchFn, 1)[0]).toBe('/api/v1/me/password'));
	});
});

describe('security page details', () => {
	afterEach(useEnglish);

	it('describes each passkey by when it was added and last used', () => {
		renderSecurity();
		const created = formatDate(ts('2026-01-01T00:00:00Z'));
		expect(
			screen.getByText(t('security-passkeys-never-used', { added: created }))
		).toBeInTheDocument();
		expect(
			screen.getByText(
				t('security-passkeys-used', {
					added: created,
					used: formatRelative(ts('2026-01-02T00:00:00Z'))
				})
			)
		).toBeInTheDocument();
	});

	it('mentions two-step sign-in in the passkey card only while it is off', () => {
		renderSecurity(security({ mfaEnabled: false, totpEnabled: false }));
		expect(
			screen.getByText(t('security-passkeys-description-enables-two-step'))
		).toBeInTheDocument();
	});

	it('words recent activity with how it was proven and who caused it', () => {
		renderSecurity();
		const card = within(
			screen
				.getByRole('heading', { level: 2, name: t('security-activity-title') })
				.closest('[data-slot="card"]') as HTMLElement
		);

		expect(card.getByText(t('audit-action-role-granted'))).toBeInTheDocument();
		expect(card.getByText('admin')).toBeInTheDocument();
		expect(card.getByText(t('security-activity-by-admin'))).toBeInTheDocument();
		expect(card.getByText(t('audit-action-sign-in-failed'))).toHaveClass('text-destructive');
		expect(card.getByText(t('audit-method-password'))).toBeInTheDocument();
		expect(card.getByText(FIREFOX_MACOS)).toBeInTheDocument();
		expect(card.getByText('192.0.2.9')).toBeInTheDocument();
		// A social sign-in names the provider.
		expect(card.getByText('GitHub')).toBeInTheDocument();
		expect(card.queryByRole('button', { name: t('security-activity-more') })).toBeNull();
	});

	it('loads more activity after the last event', async () => {
		const fetchFn = mockFetch(
			reply(AuditEventPageSchema, {
				items: [{ id: 'e0', action: 'registered', occurredAt: ts('2025-12-31T00:00:00Z') }]
			})
		);
		vi.stubGlobal('fetch', fetchFn);
		renderSecurity(security(), activity({ nextCursor: 'next' }));

		await fireEvent.click(screen.getByRole('button', { name: t('security-activity-more') }));

		expect(await screen.findByText(t('audit-action-registered'))).toBeInTheDocument();
		expect(callOf(fetchFn)[0]).toBe('/api/v1/me/activity?after=next&limit=10');
		expect(screen.queryByRole('button', { name: t('security-activity-more') })).toBeNull();
	});

	it('shows that nothing happened yet', () => {
		renderSecurity(security(), activity({ items: [] }));
		expect(screen.getByText(t('security-activity-empty'))).toBeInTheDocument();
	});

	it('follows the language in use, English where a message is missing', async () => {
		renderSecurity();
		await useGerman();

		expect(
			await screen.findByRole('heading', { level: 2, name: 'Aktive Sitzungen' })
		).toBeInTheDocument();
		expect(screen.getByText('Abmelden')).toBeInTheDocument();
		// Device names follow the language; a card nobody translated stays English.
		expect(screen.getByText('Safari auf iOS')).toBeInTheDocument();
		expect(screen.getByRole('button', { name: signOutOf('Safari auf iOS') })).toBeInTheDocument();
		expect(
			screen.getByRole('heading', { level: 2, name: t('security-passkeys-title') })
		).toBeInTheDocument();
	});
});

describe('profile page', () => {
	function renderProfile(user = me()) {
		return render(ProfilePage, {
			data: rootData(user, { textChannels: [TextChannel.SMS, TextChannel.WHATSAPP] })
		});
	}

	it('saves the username and refreshes the session', async () => {
		const user = userEvent.setup();
		const updated = me();
		updated.user.username = 'alice2';
		vi.stubGlobal('fetch', mockFetch(reply(MeSchema, updated)));
		renderProfile();

		const save = screen.getByRole('button', { name: t('settings-save') });
		expect(save).toBeDisabled();
		await user.type(screen.getByLabelText(t('settings-username')), '2');
		await user.click(save);
		await vi.waitFor(() => expect(navigation.invalidate).toHaveBeenCalledWith('app:session'));
	});

	it('changes the email through a link', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(new Response(null, { status: 202 }));
		vi.stubGlobal('fetch', fetchFn);
		renderProfile();

		const success = vi.spyOn(toast, 'success');

		await user.click(screen.getByRole('button', { name: t('profile-email-change-label') }));
		await user.type(await screen.findByLabelText(t('profile-email-new')), 'new@example.com');
		// Dialogs from the base components name their close button in the reader's language.
		expect(screen.getByRole('button', { name: t('common-close') })).toBeInTheDocument();
		await user.click(screen.getByRole('button', { name: t('profile-email-send') }));
		await vi.waitFor(() => expect(callOf(fetchFn)[0]).toBe('/api/v1/me/email'));
		await vi.waitFor(() =>
			expect(success).toHaveBeenCalledWith(
				t('profile-email-link-sent', { email: 'new@example.com' })
			)
		);
		success.mockRestore();
	});

	it('adds a phone number with a texted code', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(new Response(null, { status: 202 }), reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);
		renderProfile();

		await user.click(screen.getByRole('button', { name: t('profile-phone-add-label') }));
		await user.type(await screen.findByLabelText(t('profile-phone-label')), '+49 170 1234567');
		await user.click(screen.getByRole('radio', { name: t('signin-channel-whatsapp') }));
		await user.click(screen.getByRole('button', { name: t('profile-phone-send') }));

		await user.type(await screen.findByLabelText(t('profile-phone-code-label')), '123456');
		await user.click(screen.getByRole('button', { name: t('profile-phone-verify') }));
		expect(sent(fetchFn, AddPhoneRequestSchema)).toEqual(
			create(AddPhoneRequestSchema, { phone: '+49 170 1234567', channel: TextChannel.WHATSAPP })
		);
		expect(sent(fetchFn, CodeRequestSchema, 1)).toEqual(
			create(CodeRequestSchema, { code: '123456' })
		);
		await vi.waitFor(() => expect(navigation.invalidate).toHaveBeenCalledWith('app:session'));
	});

	it('removes a phone number', async () => {
		const user = userEvent.setup();
		const withPhone = me();
		withPhone.user.phone = '+491701234567';
		withPhone.user.phoneVerified = true;
		const fetchFn = mockFetch(reply(MeSchema, me()));
		vi.stubGlobal('fetch', fetchFn);
		renderProfile(withPhone);

		const success = vi.spyOn(toast, 'success');

		await user.click(screen.getByRole('button', { name: t('profile-phone-remove-label') }));
		await vi.waitFor(() => expect(callOf(fetchFn)[1].method).toBe('DELETE'));
		await vi.waitFor(() => expect(success).toHaveBeenCalledWith(t('profile-phone-removed')));
		success.mockRestore();
	});

	it('downloads the data export as a file', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(
			new Response('{}', { headers: { 'content-type': 'application/json' } })
		);
		vi.stubGlobal('fetch', fetchFn);
		const createObjectURL = vi.fn(() => 'blob:export');
		const revokeObjectURL = vi.fn();
		// jsdom has no object URLs.
		const original = { createObjectURL: URL.createObjectURL, revokeObjectURL: URL.revokeObjectURL };
		Object.assign(URL, { createObjectURL, revokeObjectURL });
		const click = vi.spyOn(HTMLAnchorElement.prototype, 'click').mockImplementation(() => {});
		renderProfile();

		await user.click(screen.getByRole('button', { name: t('profile-data-download') }));
		await vi.waitFor(() => expect(revokeObjectURL).toHaveBeenCalledWith('blob:export'));
		expect(callOf(fetchFn)[0]).toBe('/api/v1/me/export');
		const link = click.mock.contexts[0] as HTMLAnchorElement;
		expect(link.download).toBe('personal-data.json');
		expect(link.href).toBe('blob:export');
		click.mockRestore();
		Object.assign(URL, original);
	});

	it('says so when the export fails', async () => {
		const user = userEvent.setup();
		vi.stubGlobal('fetch', mockFetch(problem(500, 'internal_error')));
		const error = vi.spyOn(toast, 'error');
		renderProfile();

		const button = screen.getByRole('button', { name: t('profile-data-download') });
		await user.click(button);
		await vi.waitFor(() => expect(error).toHaveBeenCalledWith('problem internal_error'));
		expect(button).toBeEnabled();
		error.mockRestore();
	});

	it('deletes the account after the password', async () => {
		const user = userEvent.setup();
		const fetchFn = mockFetch(problem(401, 'invalid_credentials'), noContent());
		vi.stubGlobal('fetch', fetchFn);
		renderProfile();

		await fireEvent.click(screen.getByRole('button', { name: t('settings-delete-open') }));
		const dialog = await screen.findByRole('alertdialog', {
			name: t('settings-delete-dialog-title')
		});
		const password = within(dialog).getByLabelText(t('settings-delete-password'));

		await user.click(within(dialog).getByRole('button', { name: t('settings-delete-submit') }));
		expect(password).toHaveFocus();

		await user.type(password, 'wrong');
		await user.click(within(dialog).getByRole('button', { name: t('settings-delete-submit') }));
		expect(await within(dialog).findByRole('alert')).toHaveTextContent(
			'problem invalid_credentials'
		);

		await user.clear(password);
		await user.type(password, 'secret');
		await user.click(within(dialog).getByRole('button', { name: t('settings-delete-submit') }));
		await vi.waitFor(() =>
			expect(navigation.goto).toHaveBeenCalledWith('/', { invalidateAll: true })
		);
	});
});
