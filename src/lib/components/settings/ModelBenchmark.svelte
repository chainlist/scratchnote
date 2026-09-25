<script lang="ts">
	import { benchmarkModel, downloadModel, enrichBusy } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import { Progress } from '$lib/components/ui/progress';
	import GaugeIcon from '@lucide/svelte/icons/gauge';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';

	/**
	 * Times the model in use on a few sample notes, and suggests something
	 * lighter when it is slow. Keyed on the model and the GPU switch, so a
	 * change starts over.
	 */
	let { settings, gpus }: { settings: SettingsState; gpus: string[] } = $props();

	/** Slower than this per note on average, and the benchmark says so. */
	const SLOW_MS = 5000;

	const view = $derived(settings.view!);
	const info = $derived(settings.info);

	let running = $state(false);
	/** The queue was busy, which would skew the timings, so nothing ran. */
	let busy = $state(false);
	let progress = $state<{ done: number; total: number } | null>(null);
	let loadMs = $state<number | null>(null);
	/** How long each sample that was labelled took. */
	let times = $state<number[]>([]);
	let finished = $state(false);

	const average = $derived(
		times.length ? times.reduce((sum, ms) => sum + ms, 0) / times.length : 0
	);

	const advice = $derived.by(() => {
		if (!finished) return null;
		if (times.length === 0) return 'failed';
		if (average <= SLOW_MS) return 'fast';
		if (gpus.length > 0 && !view.useGpu) return 'gpu';
		if (info?.activeVariant === 'default' && view.modelPath === null) return 'light';
		return 'slow';
	});

	async function run() {
		busy = false;
		finished = false;
		progress = null;
		loadMs = null;
		times = [];
		try {
			if (await enrichBusy()) {
				busy = true;
				return;
			}
			running = true;
			await benchmarkModel((event) => {
				if (event.kind === 'load') loadMs = event.ms;
				else {
					progress = { done: event.done, total: event.total };
					if (event.ok) times.push(event.ms);
				}
			});
			finished = true;
		} catch (e) {
			settings.say(String(e), true);
		} finally {
			running = false;
		}
	}

	async function useLight() {
		if (info?.light) {
			if (await settings.apply({ modelVariant: 'light', modelPath: null }))
				await settings.refreshModels();
			return;
		}
		try {
			await downloadModel('light');
			settings.say(m.settings_model_downloaded());
		} catch (e) {
			settings.say(String(e), true);
		}
	}

	const seconds = (ms: number) =>
		new Intl.NumberFormat(getLocale(), {
			minimumFractionDigits: 1,
			maximumFractionDigits: 1
		}).format(ms / 1000);
</script>

{#snippet status()}
	{#if running}
		{progress
			? m.settings_benchmark_progress({ done: progress.done, total: progress.total })
			: m.settings_benchmark_starting()}
	{:else if busy}
		{m.settings_benchmark_busy()}
	{:else if advice === 'failed'}
		<span class="text-amber-500">{m.settings_benchmark_failed()}</span>
	{:else if advice}
		{m.settings_benchmark_result({ seconds: seconds(average) })}
		{#if loadMs !== null}{m.settings_benchmark_load({ seconds: seconds(loadMs) })}{/if}
		{#if advice === 'fast'}
			{m.settings_benchmark_fast()}
		{:else}
			<span class="text-amber-500">
				{#if advice === 'gpu'}{m.settings_benchmark_gpu()}
				{:else if advice === 'light'}{m.settings_benchmark_light({ light: m.model_light_name() })}
				{:else}{m.settings_benchmark_slow()}{/if}
			</span>
		{/if}
	{:else}
		{m.settings_benchmark_hint()}
	{/if}
{/snippet}

<SettingRow label={m.settings_benchmark()} hint={status}>
	<div class="flex shrink-0 gap-2">
		{#if advice === 'gpu' && !running}
			<Button size="sm" onclick={() => settings.apply({ useGpu: true })}>
				{m.settings_benchmark_use_gpu()}
			</Button>
		{:else if advice === 'light' && !running}
			<Button size="sm" onclick={useLight} disabled={settings.model.state === 'downloading'}>
				{m.settings_benchmark_use_light({ light: m.model_light_name() })}
			</Button>
		{/if}
		<Button variant="secondary" size="sm" onclick={run} disabled={running}>
			<GaugeIcon class={running ? 'animate-pulse' : ''} />
			{running ? m.settings_benchmark_running() : m.settings_benchmark_run()}
		</Button>
	</div>
</SettingRow>
{#if running && progress}
	<div class="px-4 py-3"><Progress value={(progress.done / progress.total) * 100} /></div>
{/if}
