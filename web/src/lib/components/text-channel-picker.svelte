<script lang="ts">
	import { RadioGroup } from 'bits-ui';
	import * as Field from '$lib/components/ui/field';
	import { t } from '$lib/i18n';
	import { TextChannel } from '$lib/types/api';

	/**
	 * SMS or WhatsApp, when the server offers both. Renders nothing for a single channel.
	 * A radio group: Tab reaches the chosen channel, the arrow keys change it.
	 */
	let { channels, value = $bindable() }: { channels: readonly TextChannel[]; value: TextChannel } =
		$props();

	const label = (channel: TextChannel) =>
		channel === TextChannel.SMS
			? t('signin-channel-sms')
			: channel === TextChannel.WHATSAPP
				? t('signin-channel-whatsapp')
				: t('signin-channel-text');
	const labelId = $props.id();
</script>

{#if channels.length > 1}
	<Field.Field>
		<Field.Title id={labelId}>{t('signin-channel-label')}</Field.Title>
		<RadioGroup.Root
			bind:value={() => String(value), (next) => (value = Number(next) as TextChannel)}
			orientation="horizontal"
			aria-labelledby={labelId}
			class="grid grid-cols-2 gap-1 rounded-4xl bg-muted p-1"
		>
			{#each channels as channel (channel)}
				<RadioGroup.Item
					value={String(channel)}
					class="rounded-4xl px-3 py-1.5 text-sm font-medium text-muted-foreground transition-colors hover:text-foreground data-[state=checked]:bg-background data-[state=checked]:text-foreground data-[state=checked]:shadow-sm"
				>
					{label(channel)}
				</RadioGroup.Item>
			{/each}
		</RadioGroup.Root>
	</Field.Field>
{/if}
