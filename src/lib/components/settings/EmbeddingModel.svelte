<script lang="ts">
	import { onMount } from 'svelte';
	import {
		downloadEmbeddingModel,
		EMBEDDING_SIZE,
		embeddingModelInfo,
		onEmbeddingStatus,
		type EmbeddingModelInfo
	} from '$lib/api';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Progress } from '$lib/components/ui/progress';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { m } from '$lib/paraglide/messages';
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
		return () => void off.then((stop) => stop());
	});

	async function download() {
		try {
			await downloadEmbeddingModel();
			settings.say(m.settings_embedding_downloaded());
		} catch (e) {
			settings.say(String(e), true);
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
				{EMBEDDING_SIZE}
			</Button>
		{/if}
	</SettingRow>
	{#if embedding?.downloading != null}
		<div class="px-4 py-3"><Progress value={embedding.downloading} /></div>
	{/if}
</div>
