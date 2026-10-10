<script lang="ts">
	import { onMount } from 'svelte';
	import {
		downloadEmbeddingModel,
		EMBEDDING_SIZE,
		embeddingModelInfo,
		onEmbeddingStatus,
		stopAll,
		type EmbeddingModelInfo
	} from '#lib/api.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Progress } from '#lib/components/ui/progress/index.js';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { m } from '#lib/paraglide/messages.js';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	let embedding = $state<EmbeddingModelInfo | null>(null);

	onMount(() => {
		const off = onEmbeddingStatus(
			(status) =>
				(embedding = {
					installed: status.state === 'installed',
					downloading: status.state === 'downloading' ? status.percent : null
				})
		);
		embeddingModelInfo()
			.then((info) => (embedding = info))
			.catch((e) => settings.say(String(e), true));
		return stopAll(off);
	});

	async function download() {
		try {
			await downloadEmbeddingModel();
			settings.say(m.settings_embedding_downloaded());
		} catch (e) {
			settings.say(m.error_model_download({ reason: String(e) }), true);
		}
	}
</script>

<div class={group}>
	<SettingRow
		label={m.settings_embedding()}
		hint={embedding?.downloading != null
			? m.settings_embedding_downloading({ percent: embedding.downloading })
			: m.settings_embedding_hint()}
	>
		{#if embedding?.installed}
			<Badge variant="outline">{m.settings_embedding_installed()}</Badge>
		{:else}
			<Button
				variant="secondary"
				size="sm"
				onclick={download}
				disabled={!embedding || embedding.downloading != null}
			>
				<DownloadIcon />
				{m.settings_model_download({ size: EMBEDDING_SIZE })}
			</Button>
		{/if}
	</SettingRow>
	{#if embedding?.downloading != null}
		<div class="px-4 py-3">
			<Progress value={embedding.downloading} aria-label={m.settings_embedding()} />
		</div>
	{/if}
</div>
