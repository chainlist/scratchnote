<script lang="ts">
	import { shortDay } from '#lib/components/ViewHeader.svelte';
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
	const places = $derived(shell.threadsOf(id));
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
			shell.dockThread(thread.id);
		}}
		title={m.thread_open({ count: thread.notes.length, date: shortDay(thread.since) })}
		class={cn(
			'flex max-w-full min-w-0 cursor-pointer items-center gap-1.5 text-xs text-neutral-500 transition-colors hover:text-neutral-200',
			className
		)}
	>
		<RouteIcon class="size-3 shrink-0" />
		{#if thread.mention}<span class="shrink-0 text-neutral-400">@{thread.mention}</span>{/if}
		<span class="truncate">{thread.title ?? m.thread_untitled()}</span>
		<span class="shrink-0 text-neutral-600">
			{m.thread_position({ n: index + 1, count: thread.notes.length })}
		</span>
	</button>
{/each}
