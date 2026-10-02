<script lang="ts">
	import { fade } from 'svelte/transition';
	import type { Note } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import DayAhead from '#lib/components/DayAhead.svelte';
	import ThreadLine from '#lib/components/ThreadLine.svelte';
	import TimelineItem from '#lib/components/TimelineItem.svelte';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import FileXIcon from '@lucide/svelte/icons/file-x';
	import PanelRightOpenIcon from '@lucide/svelte/icons/panel-right-open';
	import { categoryLabel } from '#lib/categories.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		note,
		onopen,
		ondock,
		ondelete,
		oncategory,
		showDate = false,
		threadLine = true,
		blink = false
	}: {
		/** A page: its subject is its title (SPEC 3.5). */
		note: Note;
		onopen: (page: Note) => void;
		/** Dock the page on the right, beside the day. */
		ondock: (page: Note) => void;
		/** For a missing page, which takes its stub out of the day. */
		ondelete: (page: Note) => void;
		oncategory: (category: string) => void;
		showDate?: boolean;
		/** Name the thread the page is in, as everywhere but in that thread. */
		threadLine?: boolean;
		blink?: boolean;
	} = $props();

	const words = $derived(note.body.split(/\s+/).filter(Boolean).length);
	/** Reading at 200 words a minute, the usual rough figure; never under one. */
	const minutes = $derived(Math.max(1, Math.round(words / 200)));
	/** The start of the text without its blank lines, so both preview lines say something. */
	const preview = $derived(
		note.body
			.split('\n')
			.filter((line) => line.trim())
			.slice(0, 2)
			.join('\n')
	);
	const shell = getShell();
	/** As a note's: only while the model is there to label it. */
	const glowing = $derived(note.status === 'pending' && shell.modelAvailable);
	const glowFade = { duration: 500 };
</script>

<!-- Laid out as a note card, so the page sits on the same rail at its time.
     The card is one big click target; its title is the button, for the
     keyboard, and the category a button of its own. -->
<TimelineItem
	data-note-id={note.id}
	time={note.time}
	date={showDate ? note.date : undefined}
	hover={false}
	class={[blink && 'note-blink', glowing && 'z-10']}
>
	{#snippet icon()}
		{#if note.missing}<FileXIcon class="size-3" />{:else}<FileTextIcon class="size-3" />{/if}
	{/snippet}
	{#if glowing}
		<span aria-hidden="true" class="note-aurora" transition:fade={glowFade}></span>
	{:else if note.status === 'failed'}
		<span aria-hidden="true" class="note-error-ring" transition:fade={glowFade}></span>
	{/if}

	<!-- The box rises into the item's padding, so its title, inside the box's
	     own padding, comes about level with a note's first line. -->
	<div class="-my-2 min-w-0">
		{#if note.missing}
			<div class="rounded-lg border border-dashed border-neutral-800 px-4 py-3">
				<p class="truncate text-base text-neutral-400">{note.subject ?? m.pages_untitled()}</p>
				<p class="mt-1 text-sm text-neutral-600">{m.pages_missing()}</p>
				<button
					type="button"
					onclick={() => ondelete(note)}
					class="-mx-1.5 mt-2 cursor-pointer rounded px-1.5 py-0.5 text-xs text-neutral-500 hover:bg-neutral-800 hover:text-neutral-200"
				>
					{m.pages_remove_stub()}
				</button>
			</div>
		{:else}
			<!-- svelte-ignore a11y_click_events_have_key_events, a11y_no_static_element_interactions -->
			<div
				onclick={() => onopen(note)}
				class="cursor-pointer rounded-lg border border-neutral-800 px-4 py-3 transition-colors duration-300 group-hover:border-neutral-700 group-hover:bg-neutral-900"
			>
				<div class="flex items-baseline gap-3">
					<button
						type="button"
						onclick={(event) => {
							event.stopPropagation();
							onopen(note);
						}}
						class="min-w-0 flex-1 cursor-pointer truncate text-left text-base font-medium text-neutral-100 outline-none focus-visible:underline"
					>
						{note.subject}
					</button>
					<span
						class="flex shrink-0 items-center gap-2 text-xs text-neutral-500 opacity-0 transition group-focus-within:opacity-100 group-hover:opacity-100"
					>
						<button
							type="button"
							onclick={(event) => {
								event.stopPropagation();
								ondock(note);
							}}
							aria-label={m.pages_open_side()}
							title={m.pages_open_side()}
							class="cursor-pointer rounded px-1 py-0.5 text-neutral-400 outline-none hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200"
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
				<div class="mt-2 flex items-center gap-3 text-xs text-neutral-600">
					<span>{m.pages_words({ count: words })}</span>
					{#if words > 0}<span>{m.pages_read_time({ count: minutes })}</span>{/if}
					{#if note.category}
						{@const category = note.category}
						<button
							type="button"
							onclick={(event) => {
								event.stopPropagation();
								oncategory(category);
							}}
							title={m.note_show_tag({ tag: categoryLabel(category) })}
							class="cursor-pointer font-mono hover:text-neutral-200"
							>#{categoryLabel(category)}</button
						>
					{/if}
				</div>
				{#if note.on || (threadLine && shell.threadOf(note.id))}
					<div class="mt-1.5 flex min-w-0 items-center gap-3">
						{#if note.on}<DayAhead on={note.on} />{/if}
						{#if threadLine}<ThreadLine id={note.id} />{/if}
					</div>
				{/if}
			</div>
		{/if}
	</div>
</TimelineItem>
