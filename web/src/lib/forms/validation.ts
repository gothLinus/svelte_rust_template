/**
 * Client-side validation that mirrors the server's rules, so the browser can flag bad input
 * before a round trip. Messages match the server's. The server remains the authority: its
 * `422` field errors are shown the same way (see `FormState`).
 *
 * Keep the rules in sync with the value objects in `server/crates/domain/src/`; the limits
 * are generated from them.
 */

import {
	MAX_EMAIL_LENGTH,
	MAX_FILE_NAME_LENGTH,
	MAX_FILE_SIZE,
	MAX_NOTE_BODY_LENGTH,
	MAX_NOTE_TITLE_LENGTH,
	MAX_PASSKEY_NAME_LENGTH,
	MAX_PASSWORD_LENGTH,
	MAX_USERNAME_LENGTH,
	MIN_PASSWORD_LENGTH,
	MIN_USERNAME_LENGTH
} from '$lib/types/generated/limits';
import { t } from '$lib/i18n';

export {
	MAX_EMAIL_LENGTH,
	MAX_FILE_NAME_LENGTH,
	MAX_FILE_SIZE,
	MAX_NOTE_BODY_LENGTH,
	MAX_NOTE_TITLE_LENGTH,
	MAX_PASSKEY_NAME_LENGTH,
	MAX_PASSWORD_LENGTH,
	MAX_USERNAME_LENGTH,
	MIN_PASSWORD_LENGTH,
	MIN_USERNAME_LENGTH
};

export type Validator = (value: string) => string | undefined;

function length(value: string): number {
	return [...value].length;
}

const utf8 = new TextEncoder();

function bytes(value: string): number {
	return utf8.encode(value).length;
}

// Control characters, except the whitespace a textarea may legitimately contain.
// eslint-disable-next-line no-control-regex
const CONTROL = /[\u0000-\u0008\u000b\u000c\u000e-\u001f\u007f-\u009f]/;
// eslint-disable-next-line no-control-regex
const CONTROL_OR_NEWLINE = /[\u0000-\u001f\u007f-\u009f]/;
// Bidirectional embeddings, overrides and isolates: mirrors `domain::unicode::is_bidi_override`.
const BIDI_OVERRIDE = /[\u202a-\u202e\u2066-\u2069]/;
// What a file name must not contain: a path separator, a control character or a bidi override.
// eslint-disable-next-line no-control-regex
const UNSAFE_IN_FILE_NAME = /[/\\\u0000-\u001f\u007f-\u009f\u202a-\u202e\u2066-\u2069]/g;
// An extension at most this long survives shortening a name; a longer one is part of the name.
const MAX_KEPT_EXTENSION_LENGTH = 16;

export const required: Validator = (value) => (value.trim() ? undefined : t('validation-required'));

export const email: Validator = (value) => {
	const address = value.trim().toLowerCase();
	if (!address) return t('validation-required');
	const [local, domain, ...rest] = address.split('@');
	const valid =
		rest.length === 0 &&
		bytes(address) <= MAX_EMAIL_LENGTH &&
		!!local &&
		!!domain &&
		domain.includes('.') &&
		!domain.startsWith('.') &&
		!domain.endsWith('.') &&
		!domain.includes('..') &&
		!/\s/.test(address) &&
		!CONTROL_OR_NEWLINE.test(address);
	return valid ? undefined : t('validation-invalid-email');
};

export const newPassword: Validator = (value) => {
	const chars = length(value);
	if (chars === 0) return t('validation-required');
	if (chars < MIN_PASSWORD_LENGTH) return t('validation-too-short', { min: MIN_PASSWORD_LENGTH });
	if (chars > MAX_PASSWORD_LENGTH) return t('validation-too-long', { max: MAX_PASSWORD_LENGTH });
	if (!value.trim()) return t('validation-password-only-spaces');
	return undefined;
};

export const username: Validator = (value) => {
	const name = value.trim().toLowerCase();
	if (!name) return t('validation-required');
	if (name.length < MIN_USERNAME_LENGTH)
		return t('validation-too-short', { min: MIN_USERNAME_LENGTH });
	if (name.length > MAX_USERNAME_LENGTH)
		return t('validation-too-long', { max: MAX_USERNAME_LENGTH });
	if (!/^[a-z0-9][a-z0-9._-]*$/.test(name)) return t('validation-username-invalid-characters');
	if (!/[a-z]/.test(name)) return t('validation-username-needs-letter');
	return undefined;
};

export const phone: Validator = (value) => {
	const compact = value.trim().replace(/[ \-.()]/g, '');
	if (!compact) return t('validation-required');
	return /^\+[1-9][0-9]{6,14}$/.test(compact) ? undefined : t('validation-invalid-phone');
};

export const code: Validator = (value) => {
	const digits = value.replace(/\s/g, '');
	if (!digits) return t('validation-required');
	return /^[0-9]{6}$/.test(digits) ? undefined : t('validation-code');
};

export const passkeyName: Validator = (value) => {
	const name = value.trim();
	if (!name) return t('validation-required');
	if (length(name) > MAX_PASSKEY_NAME_LENGTH) {
		return t('validation-too-long', { max: MAX_PASSKEY_NAME_LENGTH });
	}
	if (CONTROL_OR_NEWLINE.test(name)) return t('validation-control-characters');
	return undefined;
};

export const noteTitle: Validator = (value) => {
	const title = value.trim();
	if (!title) return t('validation-required');
	if (length(title) > MAX_NOTE_TITLE_LENGTH) {
		return t('validation-too-long', { max: MAX_NOTE_TITLE_LENGTH });
	}
	if (CONTROL_OR_NEWLINE.test(title) || BIDI_OVERRIDE.test(title)) {
		return t('validation-single-line');
	}
	return undefined;
};

export const noteBody: Validator = (value) => {
	const body = value.trimEnd();
	if (length(body) > MAX_NOTE_BODY_LENGTH) {
		return t('validation-too-long', { max: MAX_NOTE_BODY_LENGTH });
	}
	if (CONTROL.test(body)) return t('validation-control-characters');
	return undefined;
};

export const fileName: Validator = (value) => {
	const name = value.trim();
	if (!name) return t('validation-required');
	if (length(name) > MAX_FILE_NAME_LENGTH) {
		return t('validation-too-long', { max: MAX_FILE_NAME_LENGTH });
	}
	if (
		name === '.' ||
		name === '..' ||
		/[/\\]/.test(name) ||
		CONTROL_OR_NEWLINE.test(name) ||
		BIDI_OVERRIDE.test(name)
	) {
		return t('file-name-invalid');
	}
	return undefined;
};

/**
 * The name to upload a chosen file under: what the browser named it, made to pass `fileName`.
 * Characters the server refuses become `_`, and a name over the limit is shortened in front
 * of its extension. Names are legal on the user's disk that are not here, such as `a\b`.
 */
export function uploadName(chosen: string): string {
	const name = chosen.replace(UNSAFE_IN_FILE_NAME, '_').trim();
	if (!name || name === '.' || name === '..') return t('files-unnamed');
	const characters = [...name];
	if (characters.length <= MAX_FILE_NAME_LENGTH) return name;
	const dot = characters.lastIndexOf('.');
	const extension =
		dot > 0 && characters.length - dot <= MAX_KEPT_EXTENSION_LENGTH ? characters.slice(dot) : [];
	const stem = characters
		.slice(0, MAX_FILE_NAME_LENGTH - extension.length)
		.join('')
		.trimEnd();
	return stem + extension.join('');
}

export function matches(other: () => string): Validator {
	return (value) => (value === other() ? undefined : t('validation-passwords-differ'));
}

/**
 * Runs each field's validator against its value and returns the messages of the failing
 * fields, keyed by field name. Empty when everything is valid.
 */
export function validate<F extends string>(
	values: Record<F, string>,
	rules: Partial<Record<F, Validator>>
): Partial<Record<F, string>> {
	const errors: Partial<Record<F, string>> = {};
	for (const field of Object.keys(rules) as F[]) {
		const message = rules[field]?.(values[field]);
		if (message) errors[field] = message;
	}
	return errors;
}
