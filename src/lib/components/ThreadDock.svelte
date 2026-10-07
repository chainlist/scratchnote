<script lang="ts">
	import { tick } from 'svelte';
	import { getThread, type Note, type Thread } from '#lib/api.js';
	import Dock from '#lib/components/Dock.svelte';
	import ThreadView from '#lib/components/ThreadView.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { threadName } from '#lib/threads.js';

	/** A thread in the dock, where a page docks (SPEC 6.4). */
	let { id, onclose }: { id: string; onclose: () => void } = $props();

	const shell = getShell();

	/** Undefined until read, null for a thread that is gone. */
	let found = $state<{ thread: Thread; notes: Note[] } | null>();
	const thread = $derived(found?.thread ?? null);
	const heading = $derived(thread ? threadName(thread, found?.notes[0]) : m.thread_untitled());

	let scroller = $state<HTMLElement>();

	// Docked from a note's thread line, the thread opens on that note,
	// blinking, rather than on its first, often a scroll away.
	$effect(() => {
		const note = shell.dockNote;
		if (!note || !found?.notes.some((other) => other.id === note)) return;
		shell.dockNote = null;
		void tick().then(() => shell.blink(note, scroller));
	});

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
		<!-- The title before the count: in a narrow dock the count goes, so the
		     title is never cut down to a few letters. -->
		<div class="@container flex min-w-0 flex-1 items-center gap-2">
			<RouteIcon class="size-4 shrink-0 text-muted-foreground" />
			<h2 class="min-w-0 truncate text-sm font-semibold">{heading}</h2>
			{#if thread}
				<span class="hidden min-w-0 truncate text-sm text-muted-foreground @[20rem]:inline">
					{m.thread_detail({ count: thread.notes.length, date: shortDay(thread.since) })}
				</span>
			{/if}
		</div>
	{/snippet}
	<div bind:this={scroller} class="min-h-0 flex-1 overflow-y-auto px-4 pb-16">
		<!-- Mounted again for another thread, which starts with nothing chosen. -->
		{#if found !== undefined}
			{#key found?.thread.id}<ThreadView {found} />{/key}
		{/if}
	</div>
</Dock>
