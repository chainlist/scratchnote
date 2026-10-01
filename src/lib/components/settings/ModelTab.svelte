<script lang="ts">
	import { onMount } from 'svelte';
	import { gpuDevices } from '$lib/api';
	import { Progress } from '$lib/components/ui/progress';
	import * as Select from '$lib/components/ui/select';
	import { Switch } from '$lib/components/ui/switch';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import EmbeddingModel from './EmbeddingModel.svelte';
	import ModelBenchmark from './ModelBenchmark.svelte';
	import ModelChoices from './ModelChoices.svelte';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint, section } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	// Tabs are only drawn once the settings have loaded.
	const view = $derived(settings.view!);
	const info = $derived(settings.info);
	const model = $derived(settings.model);

	let gpus = $state<string[]>([]);

	/** In seconds. Never, 0, goes last. */
	const IDLE_CHOICES = [30, 60, 300, 600, 1800, 3600];
	// One saved by hand, or carried over from minutes, shows among them.
	const idleChoices = $derived(
		[...new Set([...IDLE_CHOICES, view.idleUnloadSeconds])]
			.filter((seconds) => seconds > 0)
			.sort((a, b) => a - b)
			.concat(0)
	);

	function idleLabel(seconds: number): string {
		if (seconds === 0) return m.settings_idle_never();
		const [value, unit]: [number, string] =
			seconds % 3600 === 0
				? [seconds / 3600, 'hour']
				: seconds % 60 === 0
					? [seconds / 60, 'minute']
					: [seconds, 'second'];
		return new Intl.NumberFormat(getLocale(), {
			style: 'unit',
			unit,
			unitDisplay: 'long'
		}).format(value);
	}

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
		<section class="flex flex-col gap-3">
			<h4 class={section}>{m.settings_model_section_labeling()}</h4>
			{#if info?.activePath}
				<div class={group}>
					<!-- Timings only hold for the model and device they were taken on. -->
					{#key `${info.activePath}|${view.useGpu}`}
						<ModelBenchmark {settings} {gpus} />
					{/key}
				</div>
			{/if}
			<ModelChoices {settings} />

			{#if model.state === 'downloading'}
				<div class="flex flex-col gap-2">
					<p class={hint}>
						{m.settings_model_downloading({ percent: model.percent ?? 0 })}
					</p>
					<Progress value={model.percent ?? 0} />
				</div>
			{/if}

			{#if !info?.activePath}
				<p class={hint}>
					{#if view.modelPath !== null}
						<span class="text-amber-500">{m.settings_model_custom_missing()}</span>
					{:else}
						{m.settings_model_none()}
					{/if}
				</p>
			{/if}
		</section>

		<section class="flex flex-col gap-3">
			<h4 class={section}>{m.settings_model_section_performance()}</h4>
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
				<SettingRow label={m.settings_idle()} hint={m.settings_idle_hint()}>
					<!-- Applies at once: the idle check reads it every few seconds. -->
					<Select.Root
						type="single"
						value={String(view.idleUnloadSeconds)}
						onValueChange={(seconds) => settings.apply({ idleUnloadSeconds: Number(seconds) })}
					>
						<Select.Trigger size="sm" class="w-36" aria-label={m.settings_idle()}>
							{idleLabel(view.idleUnloadSeconds)}
						</Select.Trigger>
						<Select.Content>
							{#each idleChoices as seconds (seconds)}
								<Select.Item value={String(seconds)} label={idleLabel(seconds)} />
							{/each}
						</Select.Content>
					</Select.Root>
				</SettingRow>
			</div>
		</section>
	{/if}

	<!-- The embedding model runs whether or not the model is on. -->
	<section class="flex flex-col gap-3">
		<h4 class={section}>{m.settings_model_section_search()}</h4>
		<EmbeddingModel {settings} />
	</section>
</div>
