<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import type { Note } from '#lib/api.js';
	import Markdown from '#lib/components/editor/Markdown.svelte';
	import DayAhead from '#lib/components/note/DayAhead.svelte';
	import ThreadLine from '#lib/components/ThreadLine.svelte';
	import TimelineItem from '#lib/components/note/TimelineItem.svelte';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import FileXIcon from '@lucide/svelte/icons/file-x';
	import PanelRightOpenIcon from '@lucide/svelte/icons/panel-right-open';
	import { wordCount } from '#lib/markdown.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		note,
		onopen,
		ondock,
		ondelete,
		showDate = false,
		threadLine = true,
		timeline,
		blink = false
	}: {
		/** A page: its subject is its title (SPEC 3.5). */
		note: Note;
		onopen: (page: Note) => void;
		/** Dock the page on the right, beside the day. */
		ondock: (page: Note) => void;
		/** For a missing page, which takes its stub out of the day. */
		ondelete: (page: Note) => void;
		showDate?: boolean;
		/** Name the thread the page is in, as everywhere but in that thread. */
		threadLine?: boolean;
		blink?: boolean;
		/** How the timeline places it: the part of the day it opens, the room
		 *  above it, and a box beside it while notes are being chosen. */
		timeline?: Pick<
			ComponentProps<typeof TimelineItem>,
			'part' | 'partLevel' | 'gapMinutes' | 'aside'
		>;
	} = $props();

	const words = $derived(wordCount(note.body));
	/** The start of the text without its blank lines, so both preview lines say something. */
	const preview = $derived(
		note.body
			.split('\n')
			.filter((line) => line.trim())
			.slice(0, 2)
			.join('\n')
	);
	const shell = getShell();
</script>

<!-- Laid out as a note card, so the page sits on the same rail at its time.
     The card is one big click target; its title is the button, for the
     keyboard. -->
<TimelineItem
	{...timeline}
	data-note-id={note.id}
	aria-label={note.subject ?? undefined}
	time={note.time}
	date={showDate ? note.date : undefined}
	hover={false}
	class={[blink && 'note-blink']}
>
	{#snippet icon()}
		{#if note.missing}<FileXIcon class="size-3" />{:else}<FileTextIcon class="size-3" />{/if}
	{/snippet}
	<!-- The box rises into the item's padding, so its title, inside the box's
	     own padding, comes about level with a note's first line. -->
	<div class="-my-2 min-w-0">
		{#if note.missing}
			<div class="rounded-lg border border-dashed border-neutral-800 px-4 py-3">
				<p class="truncate text-base text-neutral-400">{note.subject ?? m.pages_untitled()}</p>
				<p class="mt-1 text-sm text-meta">{m.pages_missing()}</p>
				<button
					type="button"
					onclick={() => ondelete(note)}
					class="-mx-1.5 mt-2 cursor-pointer rounded px-1.5 py-0.5 text-xs text-meta hover:bg-neutral-800 hover:text-neutral-200"
				>
					{m.pages_remove_stub()}
				</button>
			</div>
		{:else}
			<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
			<div
				onclick={() => onopen(note)}
				class="cursor-pointer rounded-lg border border-neutral-800 px-4 py-3 transition-colors duration-300 group-hover:border-neutral-700 group-hover:bg-neutral-900 group-has-focus-visible:border-neutral-700 group-has-focus-visible:bg-neutral-900"
			>
				<div class="flex items-baseline gap-3">
					<button
						type="button"
						onclick={(event) => {
							event.stopPropagation();
							onopen(note);
						}}
						class="line-clamp-2 min-w-0 flex-1 cursor-pointer text-left text-base font-medium break-words text-neutral-100 outline-none focus-visible:underline"
					>
						{note.subject}
					</button>
					<span
						class="flex shrink-0 items-center gap-2 text-xs text-meta opacity-0 transition group-focus-within:opacity-100 group-hover:opacity-100"
					>
						<button
							type="button"
							onclick={(event) => {
								event.stopPropagation();
								ondock(note);
							}}
							aria-label={m.pages_open_side()}
							title={m.pages_open_side()}
							class="-my-1 flex size-6 cursor-pointer items-center justify-center rounded text-neutral-400 focus-ring outline-none hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200"
						>
							<PanelRightOpenIcon class="size-3.5" />
						</button>
						<span class="flex items-center gap-1">
							{m.pages_open()}<ArrowRightIcon class="size-3" />
						</span>
					</span>
				</div>
				{#if preview}
					<Markdown
						text={preview}
						links={false}
						class="mt-1 max-h-[2lh] overflow-hidden text-sm leading-6 text-neutral-400"
					/>
				{/if}
				<div class="mt-2 flex items-center gap-3 text-xs text-meta tabular-nums">
					<span>{m.pages_words({ count: words })}</span>
				</div>
				{#if note.on || (threadLine && shell.threads.threadOf(note.id))}
					<div class="mt-1.5 flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
						{#if note.on}<DayAhead on={note.on} />{/if}
						{#if threadLine}<ThreadLine id={note.id} />{/if}
					</div>
				{/if}
			</div>
		{/if}
	</div>
</TimelineItem>
