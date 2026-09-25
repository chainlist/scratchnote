<script lang="ts">
	import { onMount } from 'svelte';
	import { gpuDevices } from '$lib/api';
	import { Input } from '$lib/components/ui/input';
	import { Progress } from '$lib/components/ui/progress';
	import { Switch } from '$lib/components/ui/switch';
	import { parts, slot } from '$lib/i18n.svelte';
	import { m } from '$lib/paraglide/messages';
	import EmbeddingModel from './EmbeddingModel.svelte';
	import ModelChoices from './ModelChoices.svelte';
	import ModelUpdates from './ModelUpdates.svelte';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	// Tabs are only drawn once the settings have loaded.
	const view = $derived(settings.view!);
	const info = $derived(settings.info);
	const model = $derived(settings.model);

	let gpus = $state<string[]>([]);

	onMount(async () => {
		try {
			gpus = await gpuDevices();
		} catch (e) {
			settings.say(String(e), true);
		}
	});
</script>

<div class="flex flex-col gap-6">
	<div class={group}>
		<SettingRow
			id="model-enabled"
			label={m.settings_model_enabled()}
			hint={m.settings_model_enabled_hint()}
		>
			<!-- Applies at once: off unloads the model, on lets queued notes through. -->
			<Switch
				id="model-enabled"
				checked={view.modelEnabled}
				onCheckedChange={(modelEnabled) => settings.apply({ modelEnabled })}
			/>
		</SettingRow>
	</div>

	{#if view.modelEnabled}
		<ModelChoices {settings} />

		{#if model.state === 'downloading'}
			<div class="flex flex-col gap-2">
				<p class={hint}>
					{m.settings_model_downloading({ percent: model.percent ?? 0 })}
				</p>
				<Progress value={model.percent ?? 0} />
			</div>
		{/if}

		<p class={hint}>
			{#if info?.activePath}
				{#each parts(m.settings_model_active_path({ path: slot(info.activePath) })) as part, i (i)}
					{#if i % 2}<span class="font-mono break-all">{part}</span>{:else}{part}{/if}
				{/each}
			{:else if view.modelPath !== null}
				<span class="text-amber-500">
					{m.settings_model_custom_missing()}
				</span>
			{:else}
				{m.settings_model_none()}
			{/if}
		</p>

		{#if info?.activeVariant && view.modelPath === null}
			<!-- Another model has other revisions, so a switch drops the last check. -->
			{#key info.activeVariant}
				<ModelUpdates {settings} />
			{/key}
		{/if}

		<EmbeddingModel {settings} />

		<div class={group}>
			<SettingRow
				id="gpu"
				label={m.settings_gpu()}
				hint={gpus.length === 0
					? m.settings_gpu_none()
					: m.settings_gpu_found({ devices: gpus.join(', ') })}
			>
				<!-- Applies at once: the model is unloaded and the next note reloads it. -->
				<Switch
					id="gpu"
					checked={view.useGpu && gpus.length > 0}
					disabled={gpus.length === 0}
					onCheckedChange={(useGpu) => settings.apply({ useGpu })}
				/>
			</SettingRow>
			<SettingRow id="idle" label={m.settings_idle()} hint={m.settings_idle_hint()}>
				<div class="flex items-center gap-2">
					<Input
						id="idle"
						type="number"
						min="0"
						bind:value={settings.draft.idleUnloadMinutes}
						class="w-20 text-right font-mono"
					/>
					<span class={hint}>{m.settings_idle_unit()}</span>
				</div>
			</SettingRow>
		</div>
	{/if}
</div>
