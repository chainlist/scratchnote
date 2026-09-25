<script lang="ts">
	import { onMount } from 'svelte';
	import LoaderCircle from '@lucide/svelte/icons/loader-circle';
	import {
		enrichBusy,
		enrichProgress,
		onEnrichBusy,
		onEnrichProgress,
		type EnrichProgress,
		type ModelStatus
	} from '$lib/api';
	import * as Tooltip from '$lib/components/ui/tooltip';
	import { m } from '$lib/paraglide/messages';

	let { status }: { status: ModelStatus } = $props();

	let busy = $state(false);
	let progress = $state<EnrichProgress | null>(null);

	onMount(() => {
		const off = [
			onEnrichBusy((value) => {
				busy = value;
				if (!value) progress = null;
			}),
			onEnrichProgress((value) => (progress = value))
		];
		void (async () => {
			busy = await enrichBusy();
			progress = await enrichProgress();
		})();
		return () => off.forEach((p) => void p.then((stop) => stop()));
	});

	// The worker flags a job before it loads the model, so busy while not yet
	// loaded means the load itself is what is taking time.
	const label = $derived.by(() => {
		if (busy && status.state !== 'loaded') return m.status_loading();
		if (busy) return m.status_enriching();
		switch (status.state) {
			case 'absent':
				return m.status_absent();
			case 'downloading':
				return m.status_downloading({ percent: status.percent ?? 0 });
			case 'idle':
				return m.status_idle();
			case 'loaded':
				return m.status_ready();
			case 'disabled':
				return m.status_disabled();
		}
	});

	/** What the label means, shown on hover. */
	const explanation = $derived.by(() => {
		if (busy && status.state !== 'loaded') return m.status_loading_hint();
		if (busy)
			return progress
				? m.status_enriching_progress_hint({ current: progress.current, total: progress.total })
				: m.status_enriching_hint();
		switch (status.state) {
			case 'absent':
				return m.status_absent_hint();
			case 'downloading':
				return m.status_downloading_hint();
			case 'idle':
				return m.status_idle_hint();
			case 'loaded':
				return m.status_ready_hint();
			case 'disabled':
				return m.status_disabled_hint();
		}
	});

	const count = $derived(
		busy && status.state === 'loaded' && progress ? `${progress.current}/${progress.total}` : null
	);

	const dot = $derived(
		status.state === 'loaded'
			? 'bg-emerald-500'
			: status.state === 'absent' || status.state === 'disabled'
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
