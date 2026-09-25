<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import {
		downloadModel,
		embeddingModelInfo,
		MODEL_CHOICES,
		onEmbeddingStatus,
		type EmbeddingStatus,
		type Hardware,
		type ModelVariant
	} from '$lib/api';
	import type { SettingsState } from '$lib/components/settings/state.svelte';
	import ModelBenchmark from '$lib/components/settings/ModelBenchmark.svelte';
	import { group, hint } from '$lib/components/settings/styles';
	import { Badge } from '$lib/components/ui/badge';
	import { Button } from '$lib/components/ui/button';
	import { Label } from '$lib/components/ui/label';
	import { Progress } from '$lib/components/ui/progress';
	import * as RadioGroup from '$lib/components/ui/radio-group';
	import CircleCheckIcon from '@lucide/svelte/icons/circle-check';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { m } from '$lib/paraglide/messages';

	let {
		settings,
		hardware,
		onskip
	}: {
		settings: SettingsState;
		hardware: Hardware | null;
		/** Continue without a model. */
		onskip: () => void;
	} = $props();

	const view = $derived(settings.view!);
	const info = $derived(settings.info);
	const model = $derived(settings.model);

	// Starts on the machine's suggestion, then is the user's to change.
	let variant = $state<ModelVariant>(untrack(() => hardware?.recommended) ?? 'default');
	/** The download started from this step, while it runs. */
	let downloading = $state<ModelVariant | null>(null);
	let embedding = $state<EmbeddingStatus | null>(null);

	// The model in use leads once there is one, so a switch made from the
	// benchmark shows here too. Only a change of it moves the choice.
	const active = $derived(info?.activeVariant);
	$effect(() => {
		if (active) variant = active;
	});

	const choice = $derived(MODEL_CHOICES.find((c) => c.variant === variant)!);
	const installed = $derived(info?.[variant] != null);
	const busy = $derived(model.state === 'downloading');

	onMount(() => {
		const off = onEmbeddingStatus((status) => (embedding = status));
		embeddingModelInfo()
			.then((e) => {
				// An event may have come in first, and is newer.
				embedding ??=
					e.downloading != null
						? { state: 'downloading', percent: e.downloading }
						: { state: e.installed ? 'installed' : 'absent' };
			})
			.catch((e) => settings.say(String(e), true));
		return () => void off.then((stop) => stop());
	});

	/** The backend fetches the embedding model once this one is in. */
	async function download() {
		downloading = variant;
		try {
			// Skipped earlier and came back: a model that is off would never load.
			if (!view.modelEnabled && !(await settings.apply({ modelEnabled: true }))) return;
			await downloadModel(variant);
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			downloading = null;
		}
	}

	/** A selectable model card; highlighted while its radio is checked. */
	const card =
		'gap-3 rounded-lg border bg-card px-4 py-3 font-normal transition-colors has-data-checked:border-primary has-data-checked:bg-primary/5';
</script>

<RadioGroup.Root
	value={variant}
	onValueChange={(value) => (variant = value as ModelVariant)}
	disabled={busy}
	class="gap-2"
>
	{#each MODEL_CHOICES as option (option.variant)}
		<Label for="onboarding-{option.variant}" class="{card} flex items-center">
			<RadioGroup.Item value={option.variant} id="onboarding-{option.variant}" />
			<div class="flex flex-1 flex-col gap-0.5">
				<span class="flex items-center gap-2">
					{option.name()}
					{#if hardware?.recommended === option.variant}
						<Badge>{m.onboarding_model_suggested()}</Badge>
					{/if}
				</span>
				<span class={hint}>{option.note()}</span>
			</div>
			{#if info?.[option.variant]}
				<CircleCheckIcon class="size-4 text-primary" />
			{:else}
				<span class="font-mono text-xs text-muted-foreground">{option.size}</span>
			{/if}
		</Label>
	{/each}
</RadioGroup.Root>

{#if busy || !installed || embedding?.state === 'downloading'}
	<div class="flex flex-col gap-3">
		{#if busy}
			<div class="flex flex-col gap-2">
				<p class={hint}>
					<!-- The benchmark's switch downloads too, without saying which. -->
					{downloading
						? m.onboarding_model_downloading({ model: choice.name(), percent: model.percent ?? 0 })
						: m.settings_model_downloading({ percent: model.percent ?? 0 })}
				</p>
				<Progress value={model.percent ?? 0} />
			</div>
		{:else if !installed}
			<Button onclick={download} class="self-start">
				<DownloadIcon />
				{m.onboarding_model_download({ model: choice.name() })}
			</Button>
		{/if}

		{#if embedding?.state === 'downloading'}
			<div class="flex flex-col gap-2">
				<p class={hint}>{m.onboarding_model_search_downloading({ percent: embedding.percent })}</p>
				<Progress value={embedding.percent} />
			</div>
		{:else if busy && embedding?.state !== 'installed'}
			<p class={hint}>{m.onboarding_model_search_next()}</p>
		{/if}

		{#if busy || embedding?.state === 'downloading'}
			<p class={hint}>{m.onboarding_model_background()}</p>
		{/if}
	</div>
{/if}

<!-- Optional, the same benchmark as in settings, once the model is in use. -->
{#if !busy && installed && info?.activeVariant === variant}
	<div class={group}>
		{#key `${info.activePath}|${view.useGpu}`}
			<ModelBenchmark {settings} gpus={hardware?.gpus.map((gpu) => gpu.name) ?? []} />
		{/key}
	</div>
{/if}

{#if !info?.activePath && !busy}
	<button
		type="button"
		onclick={onskip}
		class="self-start text-xs text-muted-foreground hover:text-foreground"
	>
		{m.onboarding_skip()}
	</button>
{/if}
