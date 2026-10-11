<script lang="ts">
	import { isPage, type Note } from '#lib/api.js';
	import Markdown from '#lib/components/editor/Markdown.svelte';
	import { dayTitle, shortDay } from '#lib/helpers/dates.js';
	import { noteTitle } from '#lib/markdown.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		date,
		written,
		ahead,
		today,
		class: className = ''
	}: {
		date: string;
		/** The notes and pages written on the day, by time. */
		written: Note[];
		/** The notes that look forward to the day (SPEC 5.3). */
		ahead: Note[];
		today: string;
		class?: string;
	} = $props();

	/** How many notes the preview shows before counting the rest. */
	const SHOWN = 6;

	const title = $derived(dayTitle(date));
	const shown = $derived(written.slice(0, SHOWN));
</script>

<!-- A look at the day under the pointer, as it would open: its notes by
     time, cut short, and what looked forward to it. Only a look: a click on
     the day opens it. -->
<section class={className}>
	<h2 class="mb-3 flex items-baseline gap-2 text-sm font-medium">
		{title.weekday}<span class="font-normal text-meta">{title.date}</span>
	</h2>
	{#if written.length}
		<ul class="flex flex-col gap-3">
			{#each shown as note (note.id)}
				<li class="grid grid-cols-[2.75rem_1fr] gap-x-3">
					<span class="text-xs leading-5 text-meta tabular-nums">{note.time}</span>
					<div class="min-w-0 text-sm text-neutral-300">
						{#if isPage(note)}
							<p class="truncate font-medium text-foreground">{noteTitle(note)}</p>
						{/if}
						{#if note.body}
							<Markdown text={note.body} links={false} class="max-h-[4lh] overflow-hidden" />
						{/if}
					</div>
				</li>
			{/each}
		</ul>
		{#if written.length > shown.length}
			<p class="mt-3 pl-[3.5rem] text-xs text-meta">+{written.length - shown.length}</p>
		{/if}
	{:else if !ahead.length}
		<p class="text-sm text-meta">
			{date > today ? m.page_empty_future_day() : m.page_empty_other_day()}
		</p>
	{/if}
	{#if ahead.length}
		<h3 class="mt-5 mb-1 text-xs font-medium text-meta">{m.day_from_earlier()}</h3>
		<ul>
			{#each ahead as note (note.id)}
				<li class="grid grid-cols-[auto_1fr_auto] items-start gap-x-3 py-1.5">
					<span aria-hidden="true" class="flex h-5 items-center">
						<span class="size-2.5 rounded-full border border-neutral-500"></span>
					</span>
					<span class="line-clamp-2 text-sm text-neutral-300">{noteTitle(note)}</span>
					<span class="text-xs leading-5 text-meta tabular-nums">{shortDay(note.date)}</span>
				</li>
			{/each}
		</ul>
	{/if}
</section>
