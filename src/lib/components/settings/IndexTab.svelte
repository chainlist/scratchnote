<script lang="ts">
	import { rebuildIndex } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import { m } from '#lib/paraglide/messages.js';
	import EmbeddingModel from './EmbeddingModel.svelte';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, section } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	let rebuilding = $state(false);

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
</script>

<div class="flex flex-col gap-6">
	<div class={group}>
		<SettingRow label={m.settings_rebuild_title()} hint={m.settings_rebuild_hint()}>
			<Button variant="secondary" size="sm" onclick={rebuild} disabled={rebuilding}>
				<RefreshCwIcon class={rebuilding ? 'animate-spin' : ''} />
				{rebuilding ? m.settings_rebuilding() : m.settings_rebuild()}
			</Button>
		</SettingRow>
	</div>

	<section class="flex flex-col gap-3">
		<h4 class={section}>{m.settings_model_section_search()}</h4>
		<EmbeddingModel {settings} />
	</section>
</div>
