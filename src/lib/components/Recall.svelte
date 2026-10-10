<script lang="ts">
	import { noteTitle } from '#lib/markdown.js';
	import type { Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import { draftHints, type Note } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { dayHeading, shortDay } from '#lib/dates.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import { m } from '#lib/paraglide/messages.js';
	import { cn } from '#lib/utils.js';

	let {
		text,
		initial = '',
		exclude,
		space,
		onopen,
		onmention,
		returnFocus,
		class: className,
		children
	}: {
		/** The draft as it is being typed. */
		text: string;
		/** The text the editor opened with: nothing is looked for until the draft differs. */
		initial?: string;
		/** The note the draft is an edit of, which is not to be found. */
		exclude?: string;
		/** The space the draft goes into, when not the open one. */
		space?: string;
		/** Show the old note where it is. Left out where leaving would lose the draft. */
		onopen?: (note: Note) => void;
		/** Take the name a draft that mentions none looks like (SPEC 3.10).
		 *  Left out, none is offered. */
		onmention?: (name: string) => void;
		/** Where the focus goes back once the note is read. */
		returnFocus?: () => void;
		class?: string;
		/** What shows while nothing is found, such as a hint. */
		children?: Snippet;
	} = $props();

	/** How long typing has to stop before the draft is looked at. */
	const QUIET_MS = 700;
	/** Shorter than this, a draft says too little; the backend asks as much. */
	const MIN_CHARS = 12;

	let found = $state<Note | null>(null);
	/** The name the draft looks like, while it mentions none. */
	let named = $state<string | null>(null);
	let open = $state(false);
	/** What was last asked about, so a pause on the same text asks nothing. */
	let asked = '';
	/** Bumped by every question, so only the latest answer is shown. */
	let run = 0;

	$effect(() => {
		const draft = text.trim();
		const key = `${space ?? ''}\n${exclude ?? ''}\n${draft}`;
		if (draft.length < MIN_CHARS || draft === initial.trim()) {
			asked = key;
			run++;
			found = null;
			named = null;
			return;
		}
		if (key === asked) return;
		const timer = setTimeout(async () => {
			asked = key;
			const mine = ++run;
			const hints = await draftHints(draft, exclude, space).catch(() => null);
			if (mine !== run) return;
			found = hints?.note ?? null;
			named = hints?.mention ?? null;
		}, QUIET_MS);
		return () => clearTimeout(timer);
	});

	// A note that is no longer found takes its popover with it.
	$effect(() => {
		if (!found) open = false;
	});
</script>

{#if found || (named && onmention)}
	<span
		class={cn('flex min-w-0 items-center gap-2', className)}
		transition:fade={{ duration: 200 }}
	>
		{#if named && onmention}
			{@const name = named}
			<!-- The name the draft looks like, added at its end on a click. -->
			<button
				type="button"
				title={m.mentions_suggest_title({ name: `@${name}` })}
				onclick={() => {
					named = null;
					onmention(name);
				}}
				class="mention flex shrink-0 cursor-pointer items-center gap-0.5"
			>
				<PlusIcon class="size-3" />@{name}
			</button>
		{/if}
		{#if found}
			{@const note = found}
			<Popover.Root bind:open>
				<Popover.Trigger>
					{#snippet child({ props })}
						<button
							{...props}
							type="button"
							title={m.recall_title({ date: dayHeading(note.date) })}
							class="flex min-w-0 cursor-pointer items-center gap-1 text-left transition-colors hover:text-neutral-200 focus-visible:text-neutral-200"
						>
							<HistoryIcon class="size-3 shrink-0" />
							<span class="shrink-0">{shortDay(note.date)}</span>
							<span aria-hidden="true" class="shrink-0">·</span>
							<span class="truncate">{noteTitle(note)}</span>
						</button>
					{/snippet}
				</Popover.Trigger>
				<Popover.Content
					side="top"
					align="start"
					class="max-h-(--bits-floating-available-height) w-96 max-w-[calc(100vw-1rem)] overflow-y-auto bg-background p-3"
					onCloseAutoFocus={returnFocus &&
						((event) => {
							event.preventDefault();
							returnFocus();
						})}
				>
					<div class="flex items-baseline justify-between gap-3 text-xs text-muted-foreground">
						<span class="font-mono">{note.date} {note.time}</span>
						{#if onopen}
							<Button
								variant="ghost"
								size="sm"
								class="-my-1 h-6 px-2 text-xs"
								onclick={() => {
									open = false;
									onopen(note);
								}}>{m.recall_open()}</Button
							>
						{/if}
					</div>
					{#if note.subject}
						<p class="text-sm font-medium">{note.subject}</p>
					{/if}
					<Markdown text={note.body} links={false} class="text-sm leading-6 text-neutral-300" />
				</Popover.Content>
			</Popover.Root>
		{/if}
	</span>
{:else}
	{@render children?.()}
{/if}
