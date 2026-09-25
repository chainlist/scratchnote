<script lang="ts">
	import { rebuildIndex, regenerateAll } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import { m } from '$lib/paraglide/messages';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	const hasModel = $derived(Boolean(settings.info?.activePath));

	let rebuilding = $state(false);
	let regenerating = $state(false);
	/** The regenerate button asks once more before it rewrites every note. */
	let confirmRegenerate = $state(false);

	async function rebuild() {
		rebuilding = true;
		try {
			const count = await rebuildIndex();
			settings.say(m.settings_rebuilt({ count }));
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			rebuilding = false;
		}
	}

	async function regenerate() {
		confirmRegenerate = false;
		regenerating = true;
		try {
			const count = await regenerateAll();
			settings.say(m.settings_regenerate_queued({ count }));
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			regenerating = false;
		}
	}
</script>

<div class={group}>
	<SettingRow label={m.settings_rebuild_title()} hint={m.settings_rebuild_hint()}>
		<Button variant="secondary" size="sm" onclick={rebuild} disabled={rebuilding}>
			<RefreshCwIcon class={rebuilding ? 'animate-spin' : ''} />
			{rebuilding ? m.settings_rebuilding() : m.settings_rebuild()}
		</Button>
	</SettingRow>
</div>

<div
	class="mt-6 flex flex-col gap-3 rounded-lg border border-amber-500/40 bg-amber-500/5 px-4 py-3"
>
	<div class="flex gap-3">
		<TriangleAlertIcon class="mt-0.5 size-4 shrink-0 text-amber-500" />
		<div class="flex flex-col gap-1">
			<span class="text-sm font-medium">{m.settings_regenerate_title()}</span>
			<p class={hint}>{m.settings_regenerate_hint()}</p>
			{#if !hasModel}
				<p class="text-xs text-amber-500">{m.settings_regenerate_needs_model()}</p>
			{/if}
		</div>
	</div>
	<div class="flex justify-end gap-2">
		{#if confirmRegenerate}
			<Button variant="ghost" size="sm" onclick={() => (confirmRegenerate = false)}>
				{m.common_cancel()}
			</Button>
			<Button variant="destructive" size="sm" onclick={regenerate}>
				{m.settings_regenerate_confirm()}
			</Button>
		{:else}
			<Button
				variant="outline"
				size="sm"
				onclick={() => (confirmRegenerate = true)}
				disabled={regenerating || !hasModel}
			>
				<SparklesIcon />
				{regenerating ? m.settings_regenerating() : m.settings_regenerate()}
			</Button>
		{/if}
	</div>
</div>
