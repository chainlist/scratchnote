<script lang="ts">
	import { getThread, type Note, type Thread } from '#lib/api.js';
	import Dock from '#lib/components/Dock.svelte';
	import ThreadView from '#lib/components/ThreadView.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	/** A thread in the dock, where a page docks (SPEC 6.4). */
	let { id, onclose }: { id: string; onclose: () => void } = $props();

	const shell = getShell();

	/** Undefined until read, null for a thread that is gone. */
	let found = $state<{ thread: Thread; notes: Note[] } | null>();
	const thread = $derived(found?.thread ?? null);
	const heading = $derived(thread?.title ?? m.thread_untitled());

	// Read again as its notes or threads change, as the thread's page is.
	// A slower read for the thread docked before is dropped.
	let latest = 0;
	$effect(() => {
		void shell.reloads;
		const read = ++latest;
		getThread(id).then(
			(next) => read === latest && (found = next),
			(e) => shell.showError(String(e))
		);
	});
</script>

<Dock label={heading} {onclose}>
	{#snippet title()}
		<RouteIcon class="size-4 shrink-0 text-muted-foreground" />
		<h2 class="min-w-0 truncate text-sm font-semibold">{heading}</h2>
		{#if thread}
			<span class="shrink-0 text-sm text-muted-foreground">
				{m.thread_detail({ count: thread.notes.length, date: shortDay(thread.since) })}
			</span>
		{/if}
	{/snippet}
	<div class="min-h-0 flex-1 overflow-y-auto px-4 pb-16">
		<!-- Mounted again for another thread, which starts with nothing chosen. -->
		{#if found !== undefined}
			{#key found?.thread.id}<ThreadView {found} />{/key}
		{/if}
	</div>
</Dock>
