<script lang="ts">
	import { onMount } from 'svelte';
	import { embedderActivity, onEmbedderActivity, type EmbedderActivity } from '#lib/api.js';
	import { Progress } from '#lib/components/ui/progress/index.js';
	import { Spinner } from '#lib/components/ui/spinner/index.js';

	// A dev build's look at the embedder, so it is plain whether the model
	// loaded and the notes are being embedded. Not translated: users never see it.

	let activity = $state<EmbedderActivity>({ state: 'waiting' });
	/** When the last pass ended, as seen here. */
	let endedAt = $state<Date | null>(null);

	onMount(() => {
		// What is asked for here is older than any event heard meanwhile.
		let heard = false;
		const off = onEmbedderActivity((next) => {
			heard = true;
			activity = next;
			if (next.state === 'idle') endedAt = new Date();
		});
		void embedderActivity().then((now) => {
			if (!heard) activity = now;
		});
		return () => void off.then((stop) => stop());
	});

	const busy = $derived(['loading', 'embedding', 'threads', 'map'].includes(activity.state));
	const time = (date: Date) => date.toLocaleTimeString([], { hour12: false });
</script>

<footer
	class="flex h-6 shrink-0 items-center gap-2 border-t px-3 font-mono text-[11px] text-muted-foreground"
>
	{#if busy}
		<Spinner class="size-3" />
	{:else}
		<span
			class={[
				'size-2 rounded-full',
				activity.state === 'idle' && 'bg-emerald-500',
				activity.state === 'failed' && 'bg-red-500',
				(activity.state === 'waiting' || activity.state === 'noModel') && 'bg-neutral-500'
			]}
		></span>
	{/if}
	<span class="truncate">
		{#if activity.state === 'waiting'}
			Embedder: no pass yet
		{:else if activity.state === 'noModel'}
			Embedder: no model installed
		{:else if activity.state === 'loading'}
			Embedder: loading the model
		{:else if activity.state === 'failed'}
			Embedder: the model did not load: {activity.error}
		{:else if activity.state === 'embedding'}
			Embedder: embedding {activity.space}, {activity.done} of {activity.total}
		{:else if activity.state === 'threads'}
			Embedder: placing the notes of {activity.space} in threads
		{:else if activity.state === 'map'}
			Embedder: placing the notes of {activity.space} on the map
		{:else if activity.state === 'idle'}
			Embedder: ready, {activity.vectors} notes embedded. Last pass: {activity.embedded} embedded in
			{activity.ms} ms{endedAt ? ` at ${time(endedAt)}` : ''}
		{/if}
	</span>
	{#if activity.state === 'embedding'}
		<Progress value={activity.done} max={activity.total} class="h-1 w-24" />
	{/if}
</footer>
