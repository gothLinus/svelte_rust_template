import { describe, expect, it } from 'vitest';
import { ApiError } from '$lib/api';
import { t } from '$lib/i18n';
import { FormState, invalid, rules, validate } from '$lib/forms';
import {
	MAX_NOTE_BODY_LENGTH,
	MAX_NOTE_TITLE_LENGTH,
	MAX_PASSKEY_NAME_LENGTH,
	MAX_PASSWORD_LENGTH
} from '$lib/forms/validation';

describe('validators mirror the server', () => {
	it('usernames are handles with a letter', () => {
		expect(rules.username(' ')).toBe(t('validation-required'));
		expect(rules.username('Alice_99')).toBeUndefined();
		expect(rules.username('ab')).toBe(t('validation-too-short', { min: 3 }));
		expect(rules.username('a'.repeat(31))).toBe(t('validation-too-long', { max: 30 }));
		expect(rules.username('_alice')).toBe(t('validation-username-invalid-characters'));
		expect(rules.username('12345')).toBe(t('validation-username-needs-letter'));
	});

	it('phone numbers need a country code', () => {
		expect(rules.phone('+49 (170) 123-4567')).toBeUndefined();
		// Only ASCII spaces separate digits, as in `PhoneNumber::parse`.
		expect(rules.phone('+49\t170\u00a01234567')).toBe(t('validation-invalid-phone'));
		expect(rules.phone('')).toBe(t('validation-required'));
		expect(rules.phone('0170 1234567')).toBe(t('validation-invalid-phone'));
	});

	it('codes have six digits', () => {
		expect(rules.code('123 456')).toBeUndefined();
		expect(rules.code('')).toBe(t('validation-required'));
		expect(rules.code('12345')).toBe(t('validation-code'));
	});

	it('email', () => {
		expect(rules.email('  Alice@Example.com ')).toBeUndefined();
		expect(rules.email('')).toBe(t('validation-required'));
		for (const bad of [
			'alice',
			'@example.com',
			'alice@',
			'alice@example',
			'alice@.example.com',
			'alice@example.com.',
			'alice@exa..mple.com',
			'al ice@example.com',
			'a@b@example.com',
			`${'a'.repeat(250)}@example.com`,
			// The limit counts UTF-8 bytes, like the server: 122 two-byte characters make 256.
			`${'ä'.repeat(122)}@example.com`
		]) {
			expect(rules.email(bad), bad).toBe(t('validation-invalid-email'));
		}
		expect(rules.email(`${'ä'.repeat(121)}@example.com`)).toBeUndefined();
	});

	it('new passwords', () => {
		expect(rules.newPassword('')).toBe(t('validation-required'));
		expect(rules.newPassword('short')).toBe(t('validation-too-short', { min: 8 }));
		expect(rules.newPassword(' '.repeat(8))).toBe(t('validation-password-only-spaces'));
		expect(rules.newPassword('x'.repeat(MAX_PASSWORD_LENGTH + 1))).toBe(
			t('validation-too-long', { max: MAX_PASSWORD_LENGTH })
		);
		// Characters, not UTF-16 units, count.
		expect(rules.newPassword('😀'.repeat(8))).toBeUndefined();
		expect(rules.newPassword('correct horse')).toBeUndefined();
	});

	it('note titles and bodies', () => {
		expect(rules.noteTitle('Groceries')).toBeUndefined();
		expect(rules.noteTitle('')).toBe(t('validation-required'));
		expect(rules.noteTitle('t'.repeat(MAX_NOTE_TITLE_LENGTH + 1))).toBe(
			t('validation-too-long', { max: MAX_NOTE_TITLE_LENGTH })
		);
		expect(rules.noteTitle('two\nlines')).toBe(t('validation-single-line'));

		expect(rules.noteBody('')).toBeUndefined();
		expect(rules.noteBody('line one\n\tline two')).toBeUndefined();
		expect(rules.noteBody('b'.repeat(MAX_NOTE_BODY_LENGTH + 1))).toBe(
			t('validation-too-long', { max: MAX_NOTE_BODY_LENGTH })
		);
		expect(rules.noteBody('bell\u0007')).toBe(t('validation-control-characters'));
	});

	it('passkey names', () => {
		expect(rules.passkeyName(' MacBook ')).toBeUndefined();
		expect(rules.passkeyName('  ')).toBe(t('validation-required'));
		expect(rules.passkeyName('k'.repeat(MAX_PASSKEY_NAME_LENGTH + 1))).toBe(
			t('validation-too-long', { max: MAX_PASSKEY_NAME_LENGTH })
		);
		expect(rules.passkeyName('key\u0000')).toBe(t('validation-control-characters'));
	});

	it('lengths are worded by plural rules', () => {
		expect(t('validation-too-short', { min: 1 })).toBe('must be at least 1 character');
		expect(t('validation-too-long', { max: 1 })).toBe('must be at most 1 character');
		expect(t('validation-too-long', { max: 2 })).toBe('must be at most 2 characters');
	});

	it('required and matches', () => {
		expect(rules.required(' ')).toBe(t('validation-required'));
		expect(rules.required('x')).toBeUndefined();
		const confirm = rules.matches(() => 'secret');
		expect(confirm('secret')).toBeUndefined();
		expect(confirm('other')).toBe(t('validation-passwords-differ'));
	});

	it('validate collects every failing field', () => {
		expect(
			validate(
				{ email: 'nope', username: 'alice', password: '' },
				{ email: rules.email, username: rules.username, password: rules.required }
			)
		).toEqual({ email: t('validation-invalid-email'), password: t('validation-required') });
		expect(validate({ email: 'a@b.co' }, { email: rules.email })).toEqual({});
	});
});

describe('FormState', () => {
	it('runs the action and tracks pending', async () => {
		const form = new FormState(['title']);
		let pendingDuring = false;

		const result = await form.submit(async () => {
			pendingDuring = form.pending;
			return 42;
		});

		expect(result).toBe(42);
		expect(pendingDuring).toBe(true);
		expect(form.pending).toBe(false);
		expect(form.hasErrors).toBe(false);
	});

	it('does not run the action when validation fails', async () => {
		const form = new FormState(['title', 'body']);
		let ran = false;

		const result = await form.submit(
			async () => {
				ran = true;
			},
			{ validation: { title: t('validation-required'), body: undefined } }
		);

		expect(result).toBeUndefined();
		expect(ran).toBe(false);
		expect(form.fieldErrors.title).toBe(t('validation-required'));
		expect(form.hasErrors).toBe(true);
	});

	it('maps 422 field errors onto the fields', async () => {
		const form = new FormState(['email', 'password']);

		await form.submit(async () => {
			throw new ApiError(422, 'validation_failed', 'invalid', [
				{ field: 'email', code: 'invalid_email', message: t('validation-invalid-email') }
			]);
		});

		expect(form.fieldErrors).toEqual({ email: t('validation-invalid-email') });
		expect(form.error).toBeNull();

		form.clearField('email');
		expect(form.fieldErrors.email).toBeUndefined();
		form.clearField('password');
	});

	it('shows other failures as a form error', async () => {
		const form = new FormState([]);

		await form.submit(async () => {
			throw new ApiError(401, 'invalid_credentials', 'invalid email or password');
		});
		expect(form.error).toBe('invalid email or password');

		await form.submit(async () => {
			throw new Error('boom');
		});
		expect(form.error).toBe(t('error-unknown'));

		form.reset();
		expect(form.error).toBeNull();
		expect(form.fieldErrors).toEqual({});
	});

	it('clears old errors on the next submit', async () => {
		const form = new FormState(['title']);
		form.fail(new ApiError(409, 'conflict', 'taken'));
		expect(form.error).toBe('taken');

		await form.submit(async () => 'ok');
		expect(form.error).toBeNull();
	});

	it('shows errors for fields it does not render as the form error', () => {
		const form = new FormState(['phone']);

		form.fail(
			new ApiError(422, 'validation_failed', 'invalid', [
				{ field: 'phone', code: 'invalid_phone', message: 'enter the number' },
				{ field: 'channel', code: 'invalid', message: 'is not offered' },
				{ field: 'newPassword', code: 'too_short', message: t('validation-too-short', { min: 8 }) }
			])
		);

		expect(form.fieldErrors).toEqual({ phone: 'enter the number' });
		expect(form.error).toBe(
			[
				t('form-field-problem', { field: 'Channel', message: 'is not offered' }),
				t('form-field-problem', { field: 'New password', message: 'must be at least 8 characters' })
			].join(' ')
		);
	});

	it('ignores a second submit while one is running', async () => {
		const form = new FormState(['title']);
		let release = () => {};
		let runs = 0;
		const action = () => {
			runs += 1;
			return new Promise<string>((resolve) => (release = () => resolve('done')));
		};

		const first = form.submit(action);
		await expect(form.submit(action)).resolves.toBeUndefined();
		release();

		await expect(first).resolves.toBe('done');
		expect(runs).toBe(1);
	});

	it('shows nothing for a cancelled request', async () => {
		const form = new FormState(['title']);
		await form.submit(async () => {
			throw ApiError.aborted();
		});
		expect(form.hasErrors).toBe(false);
	});
});

describe('invalid', () => {
	it('links an input to its error', () => {
		expect(invalid('email', 'bad')).toEqual({
			'aria-invalid': true,
			'aria-describedby': 'email-error'
		});
		expect(invalid('email', undefined)).toEqual({
			'aria-invalid': undefined,
			'aria-describedby': undefined
		});
	});

	it('links an input to its description while there is no error', () => {
		expect(invalid('username', undefined, { described: true })).toEqual({
			'aria-invalid': undefined,
			'aria-describedby': 'username-description'
		});
		expect(invalid('username', 'taken', { described: true })).toMatchObject({
			'aria-describedby': 'username-error'
		});
	});
});
