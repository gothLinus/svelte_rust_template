import { t } from '$lib/i18n';
import type { AuditEvent } from '$lib/types/api';

/** Actions whose detail says how the user proved who they are. */
const PROVEN_BY = new Set(['registered', 'signed_in', 'sign_in_failed', 'reauthenticated']);
/** Actions whose detail is a provider id. */
const PROVIDER = new Set(['identity_linked', 'identity_unlinked']);
const PROVIDER_PREFIX = 'provider:';

/** How many of their own events the security page shows at a time. */
export const ACTIVITY_PAGE_SIZE = 10;

const id = (raw: string) => raw.replaceAll('_', '-');

/** What happened, in words, e.g. "Signed in". */
export function auditAction(event: AuditEvent): string {
	return t(`audit-action-${id(event.action)}`);
}

/**
 * What the event concerned, in words where the server sends a name ("Passkey", "Google"),
 * as stored otherwise (a role, a passkey's label). `providers` maps provider ids to names.
 */
export function auditDetail(
	event: AuditEvent,
	providers: readonly { id: string; name: string }[] = []
): string | undefined {
	const detail = event.detail;
	if (!detail) return undefined;
	const providerName = (provider: string) =>
		providers.find((known) => known.id === provider)?.name ?? provider;
	if (PROVIDER.has(event.action)) return providerName(detail);
	if (!PROVEN_BY.has(event.action)) return detail;
	return detail.startsWith(PROVIDER_PREFIX)
		? providerName(detail.slice(PROVIDER_PREFIX.length))
		: t(`audit-method-${id(detail)}`);
}

/** A failed attempt, which the lists set apart. */
export function isWarning(event: AuditEvent): boolean {
	return event.action === 'sign_in_failed';
}
