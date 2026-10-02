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
		/** The note or page whose thread to name, if it is in one. */
		id: string;
		class?: string;
	} = $props();

	const shell = getShell();
	const place = $derived(shell.threadOf(id));
</script>

<!-- The thread a note is in (SPEC 6.4): its title and the note's place in
     it, which open the thread. Nothing for a note on its own. -->
{#if place}
	{@const { thread, index } = place}
	<button
		type="button"
		onclick={(event) => {
			// A page's card opens the page on any other click.
			event.stopPropagation();
			void shell.showThread(thread.id);
		}}
		title={m.thread_open({ count: thread.notes.length, date: shortDay(thread.since) })}
		class={cn(
			'flex max-w-full min-w-0 cursor-pointer items-center gap-1.5 text-xs text-neutral-500 transition-colors hover:text-neutral-200',
			className
		)}
	>
		<RouteIcon class="size-3 shrink-0" />
		<span class="truncate">{thread.title ?? m.thread_untitled()}</span>
		<span class="shrink-0 text-neutral-600">
			{m.thread_position({ n: index + 1, count: thread.notes.length })}
		</span>
	</button>
{/if}
