<script lang="ts">
	import { shortDay } from '#lib/dates.js';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { cn } from '#lib/utils.js';

	let {
		id,
		class: className
	}: {
		/** The note or page whose threads to name, if it is in any. */
		id: string;
		class?: string;
	} = $props();

	const shell = getShell();
	const places = $derived(shell.threads.threadsOf(id));
</script>

<!-- The threads a note is in (SPEC 6.4), one line each: the name it was
     found among, if any, its title and the note's place in it, which dock
     the thread. Nothing for a note on its own. -->
{#each places as { thread, index } (thread.id)}
	<button
		type="button"
		onclick={(event) => {
			// A page's card opens the page on any other click.
			event.stopPropagation();
			// The dock opens on this note, where it was clicked, not the thread's first.
			shell.dockThread(thread.id, id);
		}}
		title={m.thread_open({ count: thread.notes.length, date: shortDay(thread.since) })}
		class={cn(
			'flex max-w-full min-w-0 cursor-pointer items-center gap-1.5 text-xs text-meta transition-colors hover:text-neutral-200',
			className
		)}
	>
		<RouteIcon class="size-3 shrink-0" />
		{#if thread.mention}<span class="shrink-0 text-neutral-400">@{thread.mention}</span>{/if}
		<span class="truncate">{shell.threads.nameOf(thread)}</span>
		<span class="shrink-0 tabular-nums">
			{m.thread_position({ n: index + 1, count: thread.notes.length })}
		</span>
	</button>
{/each}
