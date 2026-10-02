<script lang="ts">
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import type { CorePlugin } from '#lib/plugins/core.js';
	import { coreOn, corePlugins, plugins, setCorePlugin } from '#lib/plugins/loader.js';
	import PluginOptions from './PluginOptions.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint } from './styles';

	/** Settings > Core plugins (SPEC 3.9): each core plugin with its switch. */
	let { settings }: { settings: SettingsState } = $props();

	async function toggle(core: CorePlugin, on: boolean) {
		try {
			await setCorePlugin(core, on);
		} catch (e) {
			settings.say(String(e), true);
		}
	}
</script>

<div class={group}>
	{#each corePlugins as core (core.manifest.id)}
		{@const id = core.manifest.id}
		{@const on = coreOn(core)}
		{@const status = plugins.status[id]}
		<div class="flex items-center justify-between gap-6 px-4 py-3">
			<div class="flex min-w-0 flex-col gap-0.5">
				<span class="text-sm font-medium">{core.name()}</span>
				<p class={hint}>{core.description()}</p>
				{#if status?.state === 'failed'}
					<p class="text-xs text-destructive">
						{m.plugins_failed_because({ error: status.error })}
					</p>
				{/if}
			</div>
			<div class="flex shrink-0 items-center gap-1">
				{#if on}<PluginOptions {settings} {id} name={core.name()} />{/if}
				<!-- The switch shows what is saved, so a save that fails springs back. -->
				<Switch
					bind:checked={() => on, (next) => void toggle(core, next)}
					aria-label={core.name()}
				/>
			</div>
		</div>
	{/each}
</div>
