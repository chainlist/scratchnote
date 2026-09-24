<script lang="ts">
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import type { EnrichProgress, ModelStatus } from '$lib/api';

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

<div class="flex items-center gap-2 px-2 text-xs text-neutral-500" role="status" aria-live="polite">
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
