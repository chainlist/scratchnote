<script lang="ts">
	import { fade } from 'svelte/transition';
	import type { Note } from '$lib/api';
	import Markdown from '$lib/components/Markdown.svelte';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import FileXIcon from '@lucide/svelte/icons/file-x';
	import { categoryLabel } from '$lib/categories';
	import { m } from '$lib/paraglide/messages';

	let {
		note,
		onopen,
		ondelete,
		oncategory,
		showDate = false,
		blink = false
	}: {
		/** A page: its subject is its title (SPEC 3.5). */
		note: Note;
		onopen: (page: Note) => void;
		/** For a missing page, which takes its stub out of the day. */
		ondelete: (page: Note) => void;
		oncategory: (category: string) => void;
		showDate?: boolean;
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
	const pending = $derived(note.status === 'pending');
	const glowFade = { duration: 500 };
</script>

<!-- Laid out as a note card, so the page sits on the same rail at its time.
     The card is one big click target; its title is the button, for the
     keyboard, and the category a button of its own. -->
<article
	data-note-id={note.id}
	class="group relative isolate -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-8 rounded-lg px-3 py-3 {blink
		? 'note-blink'
		: ''} {pending ? 'z-10' : ''}"
>
	{#if pending}
		<span aria-hidden="true" class="note-aurora" transition:fade={glowFade}></span>
	{:else if note.status === 'failed'}
		<span aria-hidden="true" class="note-error-ring" transition:fade={glowFade}></span>
	{/if}
	<span
		aria-hidden="true"
		class="absolute top-0 left-[6.25rem] h-[26px] w-px bg-neutral-800 [li:first-child_&]:hidden"
	></span>
	<span
		aria-hidden="true"
		class="absolute top-[26px] left-[6.25rem] flex size-[18px] -translate-x-[8.5px] items-center justify-center rounded border border-neutral-700 bg-neutral-950 text-neutral-500 transition-colors duration-300 group-hover:border-neutral-500 group-hover:text-neutral-300"
	>
		{#if note.missing}<FileXIcon class="size-3" />{:else}<FileTextIcon class="size-3" />{/if}
	</span>
	<span
		aria-hidden="true"
		class="absolute top-[44px] bottom-0 left-[6.25rem] w-px bg-neutral-800 [li:last-child_&]:hidden"
	></span>
	<time class="pt-3 text-right font-mono text-xs leading-5 text-neutral-600">
		{#if showDate}<span class="block">{note.date}</span>{/if}{note.time}
	</time>

	<div class="min-w-0">
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
						class="flex shrink-0 items-center gap-1 text-xs text-neutral-500 opacity-0 transition group-focus-within:opacity-100 group-hover:opacity-100"
					>
						{m.pages_open()}<ArrowRightIcon class="size-3" />
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
			</div>
		{/if}
	</div>
</article>
