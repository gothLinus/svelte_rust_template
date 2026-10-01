<script lang="ts">
	import LanguagesIcon from '@lucide/svelte/icons/languages';
	import { buttonVariants } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import { chooseLanguage, languageName } from '$lib/helpers/language';
	import { i18n, t } from '$lib/i18n';
	import { cn } from '$lib/utils';
</script>

{#if i18n.locales.length > 1}
	<DropdownMenu.Root>
		<DropdownMenu.Trigger
			class={cn(buttonVariants({ variant: 'ghost', size: 'icon-sm' }))}
			aria-label={`${t('language-label')}: ${languageName(i18n.locale)}`}
		>
			<LanguagesIcon />
		</DropdownMenu.Trigger>
		<DropdownMenu.Content align="end" class="w-40">
			<DropdownMenu.RadioGroup
				value={i18n.locale}
				onValueChange={(tag) => void chooseLanguage(tag)}
				aria-label={t('language-label')}
			>
				{#each i18n.locales as tag (tag)}
					<DropdownMenu.RadioItem value={tag} lang={tag}>{languageName(tag)}</DropdownMenu.RadioItem
					>
				{/each}
			</DropdownMenu.RadioGroup>
		</DropdownMenu.Content>
	</DropdownMenu.Root>
{/if}
