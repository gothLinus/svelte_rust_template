import { fireEvent, render, screen, within } from '@testing-library/svelte';
import userEvent from '@testing-library/user-event';
import { toast } from 'svelte-sonner';
import { afterEach, describe, expect, it, vi } from 'vitest';
import BottomNav from '$lib/components/bottom-nav.svelte';
import ConfirmDialog from '$lib/components/confirm-dialog.svelte';
import CursorPagination from '$lib/components/cursor-pagination.svelte';
import EmailVerificationBanner from '$lib/components/email-verification-banner.svelte';
import SessionErrorBanner from '$lib/components/session-error-banner.svelte';
import TextChannelPicker from '$lib/components/text-channel-picker.svelte';
import ThemeToggle from '$lib/components/theme-toggle.svelte';
import RecoveryCodesDialog from '$lib/components/security/recovery-codes-dialog.svelte';
import { Spinner } from '$lib/components/ui/spinner';
import { t } from '$lib/i18n';
import { Permission, TextChannel } from '$lib/types/api';
import { me, mockFetch, noContent, useEnglish, useGerman } from '../helpers';
import { navigation, visit } from './fake-app.svelte';

describe('text channel picker', () => {
	it('is a radio group with the radio keyboard model', async () => {
		const user = userEvent.setup();
		render(TextChannelPicker, {
			channels: [TextChannel.SMS, TextChannel.WHATSAPP],
			value: TextChannel.SMS
		});

		const group = screen.getByRole('radiogroup', { name: t('signin-channel-label') });
		const sms = within(group).getByRole('radio', { name: t('signin-channel-sms') });
		const whatsapp = within(group).getByRole('radio', { name: t('signin-channel-whatsapp') });
		expect(sms).toBeChecked();

		await user.tab();
		expect(sms).toHaveFocus();
		await user.keyboard('{ArrowRight}');
		expect(whatsapp).toHaveFocus();
		expect(whatsapp).toBeChecked();
	});

	it('renders nothing for a single channel', () => {
		render(TextChannelPicker, { channels: [TextChannel.SMS], value: TextChannel.SMS });
		expect(screen.queryByRole('radiogroup')).not.toBeInTheDocument();
	});
});

describe('cursor pagination', () => {
	it('offers the pages that exist', async () => {
		const user = userEvent.setup();
		const onPrevious = vi.fn();
		const onNext = vi.fn();
		render(CursorPagination, { hasPrevious: false, hasNext: true, onPrevious, onNext });

		const nav = screen.getByRole('navigation', { name: t('pagination-label') });
		expect(within(nav).getByRole('button', { name: t('pagination-previous') })).toBeDisabled();
		await user.click(within(nav).getByRole('button', { name: t('pagination-next') }));
		expect(onNext).toHaveBeenCalledOnce();
	});

	it('hides itself on a single page', () => {
		render(CursorPagination, {
			hasPrevious: false,
			hasNext: false,
			onPrevious: vi.fn(),
			onNext: vi.fn()
		});
		expect(screen.queryByRole('navigation')).not.toBeInTheDocument();
	});
});

describe('confirm dialog', () => {
	it('runs the action only when confirmed', async () => {
		const user = userEvent.setup();
		const onconfirm = vi.fn(async () => {});
		render(ConfirmDialog, {
			open: true,
			title: 'Sign out everywhere?',
			description: 'Every device is signed out.',
			confirmLabel: 'Sign out everywhere',
			onconfirm
		});

		const dialog = screen.getByRole('alertdialog', { name: 'Sign out everywhere?' });
		expect(dialog).toHaveAccessibleDescription('Every device is signed out.');
		await user.click(within(dialog).getByRole('button', { name: 'Sign out everywhere' }));

		expect(onconfirm).toHaveBeenCalledOnce();
		await vi.waitFor(() => expect(screen.queryByRole('alertdialog')).not.toBeInTheDocument());
	});

	it('does nothing on cancel', async () => {
		const user = userEvent.setup();
		const onconfirm = vi.fn(async () => {});
		render(ConfirmDialog, {
			open: true,
			title: 'Remove?',
			description: 'Gone.',
			confirmLabel: 'Remove',
			onconfirm
		});

		await user.click(screen.getByRole('button', { name: t('common-cancel') }));
		expect(onconfirm).not.toHaveBeenCalled();
	});
});

describe('recovery codes dialog', () => {
	it('stays open until the user says they saved the codes', async () => {
		const user = userEvent.setup();
		render(RecoveryCodesDialog, { codes: ['aaaaa-bbbbb', 'ccccc-ddddd'] });

		const dialog = screen.getByRole('dialog', { name: t('security-recovery-dialog-title') });
		expect(
			within(dialog).getByRole('list', { name: t('security-recovery-list-label') })
		).toHaveTextContent('aaaaa-bbbbb');
		expect(
			within(dialog).queryByRole('button', { name: t('common-close') })
		).not.toBeInTheDocument();

		await user.keyboard('{Escape}');
		expect(screen.getByRole('dialog')).toBeInTheDocument();

		await user.click(screen.getByRole('button', { name: t('security-recovery-saved') }));
		await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
	});

	it('copies the codes', async () => {
		const user = userEvent.setup();
		const writeText = vi.spyOn(navigator.clipboard, 'writeText');
		render(RecoveryCodesDialog, { codes: ['aaaaa-bbbbb', 'ccccc-ddddd'] });

		await user.click(screen.getByRole('button', { name: t('security-recovery-copy') }));
		expect(writeText).toHaveBeenCalledWith('aaaaa-bbbbb\nccccc-ddddd');
	});
});

describe('theme toggle', () => {
	it('offers light, dark and the system setting, and says which is on', async () => {
		const user = userEvent.setup();
		render(ThemeToggle);

		const trigger = screen.getByRole('button', {
			name: t('theme-button', { mode: t('theme-system') })
		});
		const open = () => fireEvent.pointerDown(trigger, { button: 0, pointerType: 'mouse' });
		await open();
		const system = await screen.findByRole('menuitemradio', { name: t('theme-system') });
		expect(system).toHaveAttribute('aria-checked', 'true');

		await user.click(screen.getByRole('menuitemradio', { name: t('theme-dark') }));
		await vi.waitFor(() =>
			expect(trigger).toHaveAccessibleName(t('theme-button', { mode: t('theme-dark') }))
		);

		await open();
		await user.click(await screen.findByRole('menuitemradio', { name: t('theme-system') }));
		await vi.waitFor(() =>
			expect(trigger).toHaveAccessibleName(t('theme-button', { mode: t('theme-system') }))
		);
	});
});

describe('session error banner', () => {
	it('explains the problem and retries', async () => {
		const user = userEvent.setup();
		render(SessionErrorBanner, { message: 'Could not reach the server.' });

		expect(screen.getByRole('alert')).toHaveTextContent('Could not reach the server.');
		await user.click(screen.getByRole('button', { name: t('session-error-retry') }));
		expect(navigation.invalidate).toHaveBeenCalledWith('app:session');
	});
});

describe('email verification banner', () => {
	it('names the address in the sentence, emphasized, and can send the link again', async () => {
		const user = userEvent.setup();
		const success = vi.spyOn(toast, 'success');
		const fetchFn = mockFetch(noContent());
		vi.stubGlobal('fetch', fetchFn);
		render(EmailVerificationBanner, { email: 'alice@example.com' });

		expect(screen.getByText('alice@example.com').tagName).toBe('STRONG');
		expect(screen.getByRole('alert')).toHaveTextContent(
			t('email-verification-body', { email: 'alice@example.com' })
		);

		await user.click(screen.getByRole('button', { name: t('email-verification-resend') }));
		await vi.waitFor(() =>
			expect(success).toHaveBeenCalledWith(
				t('email-verification-resent', { email: 'alice@example.com' })
			)
		);
		success.mockRestore();
	});
});

describe('bottom nav', () => {
	it('shows every tab that fits', () => {
		render(BottomNav, { me: me([Permission.NOTES_READ]) });
		const nav = screen.getByRole('navigation', { name: t('nav-main') });
		expect(
			within(nav)
				.getAllByRole('link')
				.map((link) => link.textContent?.trim())
		).toEqual([t('nav-dashboard'), t('notes-nav'), t('nav-profile'), t('nav-security')]);
	});

	it('moves what does not fit into "More"', async () => {
		visit('/admin/users');
		render(BottomNav, { me: me([Permission.NOTES_READ, Permission.USERS_READ]) });

		const nav = screen.getByRole('navigation', { name: t('nav-main') });
		expect(within(nav).getAllByRole('link')).toHaveLength(3);
		const more = within(nav).getByRole('button', { name: t('nav-more') });
		// The current page is in the menu, so "More" is the current tab.
		expect(more).toHaveAttribute('aria-current', 'page');

		await fireEvent.pointerDown(more, { button: 0, pointerType: 'mouse' });
		const users = await screen.findByRole('menuitem', { name: t('nav-users') });
		await fireEvent.click(users);
		await vi.waitFor(() => expect(navigation.goto).toHaveBeenCalledWith('/admin/users'));
	});
});

describe('in another language', () => {
	afterEach(useEnglish);

	it('words a component in the language in use, and follows a switch', async () => {
		visit('/dashboard');
		render(BottomNav, { me: me([Permission.NOTES_READ]) });
		const nav = () => screen.getByRole('navigation', { name: t('nav-main') });
		expect(within(nav()).getByRole('link', { name: 'Dashboard' })).toBeInTheDocument();

		await useGerman();

		await vi.waitFor(() =>
			expect(within(nav()).getByRole('link', { name: 'Übersicht' })).toBeInTheDocument()
		);
		expect(within(nav()).getByRole('link', { name: 'Notes' })).toBeInTheDocument();
	});
});

describe('built-in texts of the base components', () => {
	it('names a spinner from the catalog', () => {
		render(Spinner);
		expect(screen.getByRole('status', { name: t('common-loading') })).toBeInTheDocument();
	});
});
