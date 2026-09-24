<script lang="ts">
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import type { EnrichProgress, ModelStatus } from '$lib/api';
	import * as Tooltip from '$lib/components/ui/tooltip';

	let {
		status,
		busy,
		progress
	}: { status: ModelStatus; busy: boolean; progress: EnrichProgress | null } = $props();

	// The worker flags a job before it loads the model, so busy while not yet
	// loaded means the load itself is what is taking time.
	const label = $derived.by(() => {
		if (busy && status.state !== 'loaded') return 'Loading model';
		if (busy) return 'Enriching notes';
		switch (status.state) {
			case 'absent':
				return 'No model installed';
			case 'downloading':
				return `Downloading model ${status.percent ?? 0}%`;
			case 'idle':
				return 'Model unloaded';
			case 'loaded':
				return 'Model ready';
		}
	});

	/** What the label means, shown on hover. */
	const explanation = $derived.by(() => {
		if (busy && status.state !== 'loaded')
			return 'Loading the model into memory. The first note after launch or after an idle unload waits for this, then labelling starts.';
		if (busy)
			return progress
				? `The model is writing a subject, summary and tags for note ${progress.current} of ${progress.total}, one at a time, in the background.`
				: 'The model is writing a subject, summary and tags for new or edited notes, one at a time, in the background.';
		switch (status.state) {
			case 'absent':
				return 'Notes are saved but not labelled until a model is installed. Pick one in Settings.';
			case 'downloading':
				return 'The model is downloading. Notes captured meanwhile are queued and labelled once it is ready.';
			case 'idle':
				return 'The model was unloaded after sitting idle, to free memory. It loads again with the next note.';
			case 'loaded':
				return 'The model is in memory with nothing left to do. New and edited notes are labelled within seconds.';
		}
	});

	const count = $derived(
		busy && status.state === 'loaded' && progress ? `${progress.current}/${progress.total}` : null
	);

	const dot = $derived(
		status.state === 'loaded'
			? 'bg-emerald-500'
			: status.state === 'absent'
				? 'bg-neutral-600'
				: 'bg-amber-500'
	);
</script>

<Tooltip.Provider delayDuration={300}>
	<Tooltip.Root>
		<Tooltip.Trigger>
			{#snippet child({ props })}
				<div
					{...props}
					class="flex cursor-default items-center gap-2 px-2 text-xs text-neutral-500"
					role="status"
					aria-live="polite"
				>
					{#if busy || status.state === 'downloading'}
						<LoaderCircle class="size-3 animate-spin text-neutral-300" />
					{:else}
						<span class="size-2 rounded-full {dot}"></span>
					{/if}
					<span class:text-neutral-300={busy}>{label}</span>
					{#if count}
						<span class="ml-auto font-mono tabular-nums">{count}</span>
					{/if}
				</div>
			{/snippet}
		</Tooltip.Trigger>
		<Tooltip.Content side="top" align="start" class="max-w-60">{explanation}</Tooltip.Content>
	</Tooltip.Root>
</Tooltip.Provider>
