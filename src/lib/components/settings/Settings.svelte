<script lang="ts">
	import { onMount } from 'svelte';
	import * as Alert from '$lib/components/ui/alert';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import BrainIcon from '@lucide/svelte/icons/brain';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import PaletteIcon from '@lucide/svelte/icons/palette';
	import SlidersHorizontalIcon from '@lucide/svelte/icons/sliders-horizontal';
	import TagIcon from '@lucide/svelte/icons/tag';
	import { m } from '$lib/paraglide/messages';
	import AliasesTab from './AliasesTab.svelte';
	import AppearanceTab from './AppearanceTab.svelte';
	import GeneralTab from './GeneralTab.svelte';
	import IndexTab from './IndexTab.svelte';
	import ModelTab from './ModelTab.svelte';
	import { SettingsState } from './state.svelte';
	import { hint } from './styles';

	const settings = new SettingsState();
	onMount(() => settings.start());

	const tabs = [
		{
			value: 'general',
			label: m.settings_tab_general,
			description: m.settings_general_description,
			icon: SlidersHorizontalIcon,
			content: GeneralTab
		},
		{
			value: 'appearance',
			label: m.settings_tab_appearance,
			description: m.settings_appearance_description,
			icon: PaletteIcon,
			content: AppearanceTab
		},
		{
			value: 'model',
			label: m.settings_tab_model,
			description: m.settings_model_description,
			icon: BrainIcon,
			content: ModelTab
		},
		{
			value: 'tags',
			label: m.settings_tab_tags,
			description: m.settings_aliases_description,
			icon: TagIcon,
			content: AliasesTab
		},
		{
			value: 'index',
			label: m.settings_tab_index,
			description: m.settings_index_description,
			icon: DatabaseIcon,
			content: IndexTab
		}
	];
</script>

<!-- A message belongs to the tab it came from. -->
<Tabs.Root
	value="general"
	orientation="vertical"
	class="h-full min-h-0 gap-0"
	onValueChange={() => (settings.message = null)}
>
	<aside class="flex w-48 shrink-0 flex-col gap-4 border-r bg-muted/40 p-3">
		<Dialog.Title class="px-2 pt-1">{m.common_settings()}</Dialog.Title>
		<Tabs.List class="w-full gap-0.5 bg-transparent p-0">
			{#each tabs as t (t.value)}
				<Tabs.Trigger value={t.value} class="h-8 w-full flex-none justify-start gap-2 px-2">
					<t.icon />
					{t.label()}
				</Tabs.Trigger>
			{/each}
		</Tabs.List>
	</aside>

	<div class="flex min-w-0 flex-1 flex-col">
		<div class="flex-1 overflow-y-auto p-6">
			{#if settings.message}
				<Alert.Root variant={settings.message.error ? 'destructive' : 'default'} class="mb-5">
					{#if settings.message.error}<CircleAlertIcon />{:else}<CircleCheckIcon />{/if}
					<Alert.Description>{settings.message.text}</Alert.Description>
				</Alert.Root>
			{/if}

			{#if settings.view}
				{#each tabs as t (t.value)}
					<Tabs.Content value={t.value}>
						<div class="mb-5 flex flex-col gap-1">
							<h3 class="text-base font-semibold">{t.label()}</h3>
							<p class="text-sm text-muted-foreground">{t.description()}</p>
						</div>
						<t.content {settings} />
					</Tabs.Content>
				{/each}
			{/if}
		</div>

		{#if settings.dirty}
			<div class="flex items-center justify-between gap-4 border-t bg-muted/40 px-6 py-3">
				<p class={hint}>{m.settings_unsaved()}</p>
				<Button onclick={() => settings.save()}>{m.settings_save_changes()}</Button>
			</div>
		{/if}
	</div>
</Tabs.Root>
