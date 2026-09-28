<script lang="ts">
	import { siApple, siGithub, siGoogle } from 'simple-icons';

	let { provider, class: className = 'size-4' }: { provider: string; class?: string } = $props();

	const icons: Record<string, { path: string; color: string | null }> = {
		google: { path: siGoogle.path, color: '#4285F4' },
		apple: { path: siApple.path, color: null },
		github: { path: siGithub.path, color: null }
	};
	const icon = $derived(icons[provider]);
</script>

{#if provider === 'microsoft'}
	<!-- Not in simple-icons: four squares. -->
	<svg viewBox="0 0 24 24" class={className} aria-hidden="true">
		<path fill="#F25022" d="M1 1h10.5v10.5H1z" />
		<path fill="#7FBA00" d="M12.5 1H23v10.5H12.5z" />
		<path fill="#00A4EF" d="M1 12.5h10.5V23H1z" />
		<path fill="#FFB900" d="M12.5 12.5H23V23H12.5z" />
	</svg>
{:else if icon}
	<svg viewBox="0 0 24 24" class={className} aria-hidden="true">
		<path d={icon.path} fill={icon.color ?? 'currentColor'} />
	</svg>
{/if}
