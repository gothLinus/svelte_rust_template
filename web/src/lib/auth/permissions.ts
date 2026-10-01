import type { SignedIn } from '$lib/api';
import { type Note, type StoredFile, Permission } from '$lib/types/api';

/**
 * Permission checks for the UI: which links, buttons and pages to show.
 *
 * These mirror the server's policies (`server/crates/application/src/*\/policy.rs`) so users
 * do not see actions that would fail, but they protect nothing. The server checks every
 * request again and is the only source of truth.
 */

export type Action = 'read' | 'create' | 'update' | 'delete';

/**
 * Mirrors one server policy: whether `me` may do `action`, on `resource` when given.
 * Without a resource, `read` means "list" and `create` "create one".
 */
export type Policy<R> = (me: SignedIn | null | undefined, action: Action, resource?: R) => boolean;

export interface Owned {
	ownerId: string;
}

export function hasPermission(me: SignedIn | null | undefined, permission: Permission): boolean {
	return me?.permissions.includes(permission) ?? false;
}

export function hasAnyPermission(
	me: SignedIn | null | undefined,
	...permissions: Permission[]
): boolean {
	return permissions.some((permission) => hasPermission(me, permission));
}

/**
 * The usual rule for owned resources: `any` covers everyone's, `own` covers the user's own.
 * Mirrors `owner_or` in `application::policy`.
 */
export function ownerOr(
	me: SignedIn | null | undefined,
	resource: Owned,
	own: Permission,
	any: Permission
): boolean {
	if (!me) return false;
	return hasPermission(me, any) || (hasPermission(me, own) && resource.ownerId === me.user.id);
}

export const notePolicy: Policy<Note> = (me, action, note) => {
	switch (action) {
		case 'read':
			return note
				? ownerOr(me, note, Permission.NOTES_READ, Permission.NOTES_MANAGE)
				: hasAnyPermission(me, Permission.NOTES_READ, Permission.NOTES_MANAGE);
		case 'create':
			return hasPermission(me, Permission.NOTES_WRITE);
		case 'update':
		case 'delete':
			return note ? ownerOr(me, note, Permission.NOTES_WRITE, Permission.NOTES_MANAGE) : false;
	}
};

export const filePolicy: Policy<StoredFile> = (me, action, file) => {
	switch (action) {
		case 'read':
			return file
				? ownerOr(me, file, Permission.FILES_READ, Permission.FILES_MANAGE)
				: hasAnyPermission(me, Permission.FILES_READ, Permission.FILES_MANAGE);
		case 'create':
			return hasPermission(me, Permission.FILES_WRITE);
		case 'update':
		case 'delete':
			return file ? ownerOr(me, file, Permission.FILES_WRITE, Permission.FILES_MANAGE) : false;
	}
};
