<script lang="ts">
	import MonitorIcon from '@lucide/svelte/icons/monitor';
	import MoonIcon from '@lucide/svelte/icons/moon';
	import SunIcon from '@lucide/svelte/icons/sun';
	import { setMode, userPrefersMode } from 'mode-watcher';
	import { buttonVariants } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { t } from '$lib/i18n';
	import { cn } from '$lib/utils';

	const modes = [
		{ value: 'light', icon: SunIcon },
		{ value: 'dark', icon: MoonIcon },
		{ value: 'system', icon: MonitorIcon }
	] as const;
	type Mode = (typeof modes)[number]['value'];

	const label = (mode: Mode) => t(`theme-${mode}`);
	const current = $derived(userPrefersMode.current);
</script>

<DropdownMenu.Root>
	<DropdownMenu.Trigger
		class={cn(buttonVariants({ variant: 'ghost', size: 'icon-sm' }), 'relative')}
		aria-label={t('theme-button', { mode: label(current) })}
	>
		<SunIcon class="scale-100 rotate-0 transition-transform dark:scale-0 dark:-rotate-90" />
		<MoonIcon
			class="absolute scale-0 rotate-90 transition-transform dark:scale-100 dark:rotate-0"
		/>
	</DropdownMenu.Trigger>
	<DropdownMenu.Content align="end" class="w-36">
		<DropdownMenu.RadioGroup
			value={userPrefersMode.current}
			onValueChange={(mode) => setMode(mode as Mode)}
			aria-label={t('theme-label')}
		>
			{#each modes as mode (mode.value)}
				<DropdownMenu.RadioItem value={mode.value}>
					<mode.icon />
					{label(mode.value)}
				</DropdownMenu.RadioItem>
			{/each}
		</DropdownMenu.RadioGroup>
	</DropdownMenu.Content>
</DropdownMenu.Root>
