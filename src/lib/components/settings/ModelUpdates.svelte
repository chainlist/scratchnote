<script lang="ts">
	import { onMount } from 'svelte';
	import { checkModelUpdate, onModelUpdateProgress, updateModel, type UpdateCheck } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import { m } from '$lib/paraglide/messages';
	import type { SettingsState } from './state.svelte';

	/**
	 * Checks the model in use against its published revision, on its card.
	 * What a check finds is said above the tabs. Keyed on the model, so a
	 * switch starts over.
	 */
	let { settings }: { settings: SettingsState } = $props();

	let update = $state<UpdateCheck | null>(null);
	let checking = $state(false);
	/** Download progress of an update, null when none is running. */
	let updating = $state<number | null>(null);

	onMount(() => {
		const off = onModelUpdateProgress((percent) => (updating = percent));
		return () => void off.then((stop) => stop());
	});

	const short = (revision: string) => revision.slice(0, 7);

	async function check() {
		checking = true;
		update = null;
		try {
			update = await checkModelUpdate();
			if (update.state === 'upToDate')
				settings.say(m.settings_up_to_date({ revision: short(update.revision) }));
			else if (update.state === 'newer') settings.say(newer(update));
			else settings.say(m.settings_update_failed({ reason: update.reason }), true);
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			checking = false;
		}
	}

	function newer(found: Extract<UpdateCheck, { state: 'newer' }>) {
		return m.settings_update_newer({
			latest: short(found.latest),
			installed: short(found.installed)
		});
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
</script>

{#if updating !== null}
	<Button size="sm" disabled>
		<DownloadIcon class="animate-pulse" />
		{m.settings_update_progress({ percent: updating })}
	</Button>
{:else if update?.state === 'newer'}
	<Button size="sm" onclick={apply} title={newer(update)}>
		<DownloadIcon />
		{m.settings_update()}
	</Button>
{:else}
	<Button variant="secondary" size="sm" onclick={check} disabled={checking}>
		<RefreshCwIcon class={checking ? 'animate-spin' : ''} />
		{checking ? m.settings_checking() : m.settings_check_updates()}
	</Button>
{/if}
