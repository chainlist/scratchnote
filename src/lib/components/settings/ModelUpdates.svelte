<script lang="ts">
	import { onMount } from 'svelte';
	import { checkModelUpdate, onModelUpdateProgress, updateModel, type UpdateCheck } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import { Progress } from '$lib/components/ui/progress';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import { m } from '$lib/paraglide/messages';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group } from './styles';

	/** Checks the model in use against its published revision. Keyed on it, so a switch starts over. */
	let { settings }: { settings: SettingsState } = $props();

	let update = $state<UpdateCheck | null>(null);
	let checking = $state(false);
	/** Download progress of an update, null when none is running. */
	let updating = $state<number | null>(null);

	onMount(() => {
		const off = onModelUpdateProgress((percent) => (updating = percent));
		return () => void off.then((stop) => stop());
	});

	async function check() {
		checking = true;
		update = null;
		try {
			update = await checkModelUpdate();
		} finally {
			checking = false;
		}
	}

	async function apply() {
		updating = 0;
		try {
			await updateModel();
			update = null;
			await settings.refreshModels();
			settings.say(m.settings_model_updated());
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			updating = null;
		}
	}

	const short = (revision: string) => revision.slice(0, 7);
</script>

{#snippet status()}
	{#if updating !== null}
		{m.settings_update_downloading({ percent: updating })}
	{:else if update?.state === 'upToDate'}
		{m.settings_up_to_date({ revision: short(update.revision) })}
	{:else if update?.state === 'newer'}
		{m.settings_update_newer({
			latest: short(update.latest),
			installed: short(update.installed)
		})}
	{:else if update?.state === 'failed'}
		<span class="text-amber-500">{m.settings_update_failed({ reason: update.reason })}</span>
	{:else}
		{m.settings_update_hint()}
	{/if}
{/snippet}

<div class={group}>
	<SettingRow label={m.settings_updates()} hint={status}>
		{#if update?.state === 'newer' && updating === null}
			<Button size="sm" onclick={apply}>
				<DownloadIcon />
				{m.settings_update()}
			</Button>
		{:else}
			<Button
				variant="secondary"
				size="sm"
				onclick={check}
				disabled={checking || updating !== null}
			>
				<RefreshCwIcon class={checking ? 'animate-spin' : ''} />
				{checking ? m.settings_checking() : m.settings_check_updates()}
			</Button>
		{/if}
	</SettingRow>
	{#if updating !== null}
		<div class="px-4 py-3"><Progress value={updating} /></div>
	{/if}
</div>
