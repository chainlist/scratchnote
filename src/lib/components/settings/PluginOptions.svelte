<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import { m } from '#lib/paraglide/messages.js';
	import { registry } from '#lib/plugins/registry.svelte.js';
	import type { SettingsState } from './state.svelte';

	/** The gear on a plugin's row, opening its own tab when it has one. */
	let { settings, id, name }: { settings: SettingsState; id: string; name: string } = $props();

	const hasTab = $derived(registry.settingTabs.some((entry) => entry.plugin === id));
</script>

{#if hasTab}
	<Button
		variant="ghost"
		size="icon-sm"
		onclick={() => (settings.tab = `plugin:${id}`)}
		aria-label={m.plugins_options({ name })}
		title={m.plugins_options({ name })}
		class="text-muted-foreground hover:text-foreground"
	>
		<SettingsIcon />
	</Button>
{/if}
