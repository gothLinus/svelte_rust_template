<script lang="ts">
	import { toast } from 'svelte-sonner';
	import { Button } from '$lib/components/ui/button';
	import * as Card from '$lib/components/ui/card';
	import { chooseLanguage, languageName } from '$lib/helpers/language';
	import { i18n, t } from '$lib/i18n';

	let pending = $state(false);

	async function choose(tag: string) {
		if (tag === i18n.locale) return;
		pending = true;
		try {
			if (await chooseLanguage(tag)) toast.success(t('profile-language-saved'));
		} finally {
			pending = false;
		}
	}
</script>

{#if i18n.locales.length > 1}
	<Card.Root size="sm">
		<Card.Header>
			<Card.Title level={2}>{t('profile-language-title')}</Card.Title>
			<Card.Description>{t('profile-language-description')}</Card.Description>
		</Card.Header>
		<Card.Content>
			<div role="radiogroup" aria-label={t('language-label')} class="flex flex-wrap gap-2">
				{#each i18n.locales as tag (tag)}
					{@const current = tag === i18n.locale}
					<Button
						role="radio"
						aria-checked={current}
						lang={tag}
						size="sm"
						variant={current ? 'secondary' : 'outline'}
						disabled={pending}
						onclick={() => choose(tag)}
					>
						{languageName(tag)}
					</Button>
				{/each}
			</div>
		</Card.Content>
	</Card.Root>
{/if}
