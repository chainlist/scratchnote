<script lang="ts">
	import { untrack } from 'svelte';
	import { downloadModel, MODEL_CHOICES, type ModelVariant } from '#lib/api.js';
	import { Badge } from '#lib/components/ui/badge/index.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import * as RadioGroup from '#lib/components/ui/radio-group/index.js';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { m } from '#lib/paraglide/messages.js';
	import ModelUpdates from './ModelUpdates.svelte';
	import type { SettingsState } from './state.svelte';
	import { hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	// Tabs are only drawn once the settings have loaded.
	const view = $derived(settings.view!);
	const info = $derived(settings.info);

	// Starts from the saved path, then is the user's to type in.
	let customPath = $state(untrack(() => settings.view?.modelPath) ?? '');

	const choice = $derived(view.modelPath !== null ? 'custom' : (info?.activeVariant ?? ''));

	/** Applies at once: the model is unloaded and the next note loads this one. */
	async function use(modelVariant: ModelVariant, modelPath: string | null) {
		if (await settings.apply({ modelVariant, modelPath })) await settings.refreshModels();
	}

	function pick(value: string) {
		if (value === 'custom') void use(view.modelVariant, customPath);
		else void use(value as ModelVariant, null);
	}

	async function download(variant: ModelVariant) {
		try {
			await downloadModel(variant);
			settings.say(m.settings_model_downloaded());
		} catch (e) {
			settings.say(String(e), true);
		}
	}

	const short = (revision: string) => revision.slice(0, 7);

	/** A selectable model card; highlighted while its radio is checked. */
	const card =
		'gap-3 rounded-lg border bg-card px-4 py-3 font-normal transition-colors has-data-checked:border-primary has-data-checked:bg-primary/5';
</script>

<RadioGroup.Root value={choice} onValueChange={pick} class="gap-2">
	{#each MODEL_CHOICES as option (option.variant)}
		{@const record = info?.[option.variant] ?? null}
		<Label for="model-{option.variant}" class="{card} flex items-center">
			<RadioGroup.Item value={option.variant} id="model-{option.variant}" disabled={!record} />
			<div class="flex flex-1 flex-col gap-0.5">
				<span class="flex items-center gap-2">
					{option.name()}
					{#if info?.activeVariant === option.variant && view.modelPath === null}
						<Badge>{m.settings_model_in_use()}</Badge>
					{/if}
				</span>
				<span class={hint}>{option.note()}</span>
			</div>
			{#if record && info?.activeVariant === option.variant && view.modelPath === null}
				<!-- On the model in use only, so a switch drops the last check. -->
				<ModelUpdates {settings} />
			{:else if record}
				<Badge variant="outline" class="font-mono" title={record.revision}>
					{short(record.revision)}
				</Badge>
			{:else}
				<Button
					variant="secondary"
					size="sm"
					onclick={() => download(option.variant)}
					disabled={settings.model.state === 'downloading'}
				>
					<DownloadIcon />
					{option.size}
				</Button>
			{/if}
		</Label>
	{/each}

	<div class="{card} flex flex-col">
		<div class="flex items-center gap-3">
			<RadioGroup.Item
				value="custom"
				id="model-custom"
				disabled={view.modelPath === null && customPath.trim() === ''}
			/>
			<Label for="model-custom" class="flex-1 font-normal">
				{m.settings_model_custom()}
				{#if view.modelPath !== null}<Badge>{m.settings_model_in_use()}</Badge>{/if}
			</Label>
		</div>
		<div class="flex gap-2 pl-7">
			<Input
				bind:value={customPath}
				placeholder="C:\models\my-model.gguf"
				spellcheck="false"
				class="font-mono"
			/>
			<Button
				variant="secondary"
				onclick={() => use(view.modelVariant, customPath)}
				disabled={customPath.trim() === '' || customPath === view.modelPath}
			>
				{m.settings_model_use()}
			</Button>
		</div>
	</div>
</RadioGroup.Root>
