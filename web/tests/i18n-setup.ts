import { i18n } from '$lib/i18n';

// Tests read expected text from the catalog (`t('login-title')`) instead of repeating it,
// and a message that is missing must fail the test that renders it, not show its id.
i18n.strict = true;
