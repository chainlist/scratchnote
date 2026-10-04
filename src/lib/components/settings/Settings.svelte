<script lang="ts">
	import { onMount } from 'svelte';
	import * as Alert from '#lib/components/ui/alert/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Tabs from '#lib/components/ui/tabs/index.js';
	import BlocksIcon from '@lucide/svelte/icons/blocks';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import DatabaseIcon from '@lucide/svelte/icons/database';
	import PackageIcon from '@lucide/svelte/icons/package';
	import PaletteIcon from '@lucide/svelte/icons/palette';
	import RouteIcon from '@lucide/svelte/icons/route';
	import SlidersHorizontalIcon from '@lucide/svelte/icons/sliders-horizontal';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { coreIds, pluginDescription, pluginName } from '#lib/plugins/loader.js';
	import { registry } from '#lib/plugins/registry.svelte.js';
	import AppearanceTab from './AppearanceTab.svelte';
	import CommunityPluginsTab from './CommunityPluginsTab.svelte';
	import CorePluginsTab from './CorePluginsTab.svelte';
	import GeneralTab from './GeneralTab.svelte';
	import IndexTab from './IndexTab.svelte';
	import PluginSettingsHost from './PluginSettingsHost.svelte';
	import ThreadsTab from './ThreadsTab.svelte';
	import { SettingsState } from './state.svelte';
	import { hint, section } from './styles';

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
			value: 'index',
			label: m.settings_tab_index,
			description: m.settings_index_description,
			icon: DatabaseIcon,
			content: IndexTab
		},
		{
			value: 'threads',
			// The same word in every language.
			label: () => 'Threads',
			description: m.settings_threads_description,
			icon: RouteIcon,
			content: ThreadsTab
		},
		{
			value: 'core-plugins',
			label: m.plugins_group_core,
			description: m.settings_core_plugins_description,
			icon: PackageIcon,
			content: CorePluginsTab
		},
		{
			value: 'community-plugins',
			label: m.plugins_group_community,
			description: m.settings_community_plugins_description,
			icon: BlocksIcon,
			content: CommunityPluginsTab
		}
	];

	/** The plugins' own tabs, core ones and community ones apart, as in Obsidian. */
	const coreTabs = $derived(registry.settingTabs.filter((entry) => coreIds.has(entry.plugin)));
	const communityTabs = $derived(
		registry.settingTabs.filter((entry) => !coreIds.has(entry.plugin))
	);

	// A plugin's tab goes with the plugin; the settings go back to its list.
	$effect(() => {
		const shown = settings.tab;
		if (!shown.startsWith('plugin:')) return;
		const id = shown.slice('plugin:'.length);
		if (!registry.settingTabs.some((entry) => entry.plugin === id))
			settings.tab = coreIds.has(id) ? 'core-plugins' : 'community-plugins';
	});
</script>

{#snippet pluginTriggers(heading: string, entries: typeof registry.settingTabs)}
	{#if entries.length > 0}
		<div class="flex flex-col gap-1">
			<p class="px-2 {section}">{heading}</p>
			<Tabs.List class="w-full gap-0.5 bg-transparent p-0">
				{#each entries as entry (entry)}
					<Tabs.Trigger
						value="plugin:{entry.plugin}"
						class="h-8 w-full flex-none justify-start gap-2 px-2"
					>
						<PluginIcon icon={entry.tab.icon} />
						<span class="truncate">{pluginName(entry.plugin)}</span>
					</Tabs.Trigger>
				{/each}
			</Tabs.List>
		</div>
	{/if}
{/snippet}

<!-- A message belongs to the tab it came from. -->
<Tabs.Root
	bind:value={settings.tab}
	orientation="vertical"
	class="h-full min-h-0 gap-0"
	onValueChange={() => (settings.message = null)}
>
	<aside class="flex w-64 shrink-0 flex-col gap-4 overflow-y-auto border-r bg-muted/40 p-3">
		<Dialog.Title class="px-2 pt-1">{m.common_settings()}</Dialog.Title>
		<Tabs.List class="w-full gap-0.5 bg-transparent p-0">
			{#each tabs as t (t.value)}
				<Tabs.Trigger value={t.value} class="h-8 w-full flex-none justify-start gap-2 px-2">
					<t.icon />
					<span class="truncate">{t.label()}</span>
				</Tabs.Trigger>
			{/each}
		</Tabs.List>
		<!-- The plugins' own tabs stand a little apart from the app's. -->
		<div class="mt-2 flex flex-col gap-4">
			{@render pluginTriggers(m.plugins_group_core(), coreTabs)}
			{@render pluginTriggers(m.plugins_group_community(), communityTabs)}
		</div>
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
			{#each registry.settingTabs as entry (entry)}
				{@const value = `plugin:${entry.plugin}`}
				<Tabs.Content {value}>
					<div class="mb-5 flex flex-col gap-1">
						<h3 class="text-base font-semibold">{pluginName(entry.plugin)}</h3>
						<p class="text-sm text-muted-foreground">{pluginDescription(entry.plugin)}</p>
					</div>
					<!-- Drawn while it shows, as Obsidian draws a plugin's tab. -->
					{#if settings.tab === value}
						<PluginSettingsHost
							tab={entry.tab}
							plugin={entry.plugin}
							onerror={(message) => settings.say(message, true)}
						/>
					{/if}
				</Tabs.Content>
			{/each}
		</div>

		{#if settings.dirty}
			<div class="flex items-center justify-between gap-4 border-t bg-muted/40 px-6 py-3">
				<p class={hint}>{m.settings_unsaved()}</p>
				<Button onclick={() => settings.save()}>{m.settings_save_changes()}</Button>
			</div>
		{/if}
	</div>
</Tabs.Root>
