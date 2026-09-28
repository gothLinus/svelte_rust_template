<script lang="ts">
	import type { HTMLInputAttributes } from 'svelte/elements';
	import { Input } from '$lib/components/ui/input';
	import { t } from '$lib/i18n';

	type Props = Omit<HTMLInputAttributes, 'type' | 'value' | 'files'> & { value?: string };

	let { value = $bindable(''), class: className, ...restProps }: Props = $props();
</script>

<Input
	type="text"
	inputmode="numeric"
	autocomplete="one-time-code"
	maxlength={7}
	placeholder={t('signin-code-placeholder')}
	spellcheck={false}
	bind:value
	class={['font-mono tracking-widest', className]}
	{...restProps}
/>
