/**
 * The attributes that tie an input to its `FormField` error or description, for screen
 * readers. Pass `described` when the `FormField` has a `description`.
 */
export function invalid(id: string, error: string | undefined, { described = false } = {}) {
	if (error) return { 'aria-invalid': true as const, 'aria-describedby': `${id}-error` };
	return {
		'aria-invalid': undefined,
		'aria-describedby': described ? `${id}-description` : undefined
	};
}
