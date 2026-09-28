<script lang="ts">
	import { encode } from 'uqr';

	let {
		value,
		label,
		class: className
	}: { value: string; label: string; class?: string } = $props();

	const qr = $derived(encode(value, { ecc: 'M', border: 2 }));
	const path = $derived(
		qr.data.flatMap((row, y) => row.map((dark, x) => (dark ? `M${x} ${y}h1v1h-1z` : ''))).join('')
	);
</script>

<svg
	viewBox="0 0 {qr.size} {qr.size}"
	class={['rounded-xl bg-white', className]}
	role="img"
	aria-label={label}
	shape-rendering="crispEdges"
>
	<path d={path} fill="#000" />
</svg>
