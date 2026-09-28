<script lang="ts">
	import EyeIcon from '@lucide/svelte/icons/eye';
	import EyeOffIcon from '@lucide/svelte/icons/eye-off';
	import type { HTMLInputAttributes } from 'svelte/elements';
	import { Input } from '$lib/components/ui/input';
	import { t } from '$lib/i18n';

	type Props = Omit<HTMLInputAttributes, 'type' | 'value' | 'files'> & { value?: string };

	let { value = $bindable(''), class: className, ...restProps }: Props = $props();

	let visible = $state(false);
</script>

<!-- A toggle button: the name stays the same and `aria-pressed` says whether it is on. -->
<div class="relative">
	<Input
		type={visible ? 'text' : 'password'}
		bind:value
		class={['pr-10', className]}
		autocapitalize="off"
		spellcheck={false}
		{...restProps}
	/>
	<button
		type="button"
		class="absolute inset-y-0 right-0 flex w-10 items-center justify-center rounded-r-3xl text-muted-foreground hover:text-foreground"
		aria-label={t('signin-show-password')}
		aria-pressed={visible}
		onclick={() => (visible = !visible)}
	>
		{#if visible}<EyeOffIcon class="size-4" />{:else}<EyeIcon class="size-4" />{/if}
	</button>
</div>
