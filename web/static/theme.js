// Applies the saved colour scheme before the first paint, so dark mode does not flash light.
// A file rather than an inline script so the Content-Security-Policy needs no hash for it.
// Keep the storage key in sync with `<ModeWatcher>` in src/routes/+layout.svelte.
(() => {
	let mode = 'system';
	try {
		mode = localStorage.getItem('mode-watcher-mode') ?? mode;
	} catch {
		// Storage is blocked: fall back to the system preference.
	}
	const dark =
		mode === 'dark' || (mode === 'system' && matchMedia('(prefers-color-scheme: dark)').matches);
	document.documentElement.classList.toggle('dark', dark);
	document.documentElement.style.colorScheme = dark ? 'dark' : 'light';
})();
