<script lang="ts" module>
	import type { Note } from '#lib/api.js';

	/** What a line names a note by: its subject, or the start of its text without markup. */
	export function noteTitle(note: Pick<Note, 'subject' | 'body'>) {
		if (note.subject) return note.subject;
		const line = note.body.split('\n').find((text) => text.trim()) ?? '';
		return line.replace(/^\s*(?:[-*+]\s+(?:\[[ xX]\]\s+)?|\d+[.)]\s+|#+\s+|>\s*)/, '').trim();
	}
</script>

<script lang="ts">
	import type { Snippet } from 'svelte';
	import { fade } from 'svelte/transition';
	import { recall } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { dayHeading, shortDay } from '#lib/components/ViewHeader.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Popover from '#lib/components/ui/popover/index.js';
	import HistoryIcon from '@lucide/svelte/icons/history';
	import { m } from '#lib/paraglide/messages.js';
	import { cn } from '#lib/utils.js';

	let {
		text,
		initial = '',
		exclude,
		space,
		onopen,
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
		/** Where the focus goes back once the note is read. */
		returnFocus?: () => void;
		class?: string;
		/** What shows while no old note is found, such as a hint. */
		children?: Snippet;
	} = $props();

	/** How long typing has to stop before the draft is looked at. */
	const QUIET_MS = 700;
	/** Shorter than this, a draft says too little; the backend asks as much. */
	const MIN_CHARS = 12;

	let found = $state<Note | null>(null);
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
			return;
		}
		if (key === asked) return;
		const timer = setTimeout(async () => {
			asked = key;
			const mine = ++run;
			const note = await recall(draft, exclude, space).catch(() => null);
			if (mine === run) found = note;
		}, QUIET_MS);
		return () => clearTimeout(timer);
	});

	// A note that is no longer found takes its popover with it.
	$effect(() => {
		if (!found) open = false;
	});
</script>

{#if found}
	{@const note = found}
	<span class={cn('flex min-w-0', className)} transition:fade={{ duration: 200 }}>
		<Popover.Root bind:open>
			<Popover.Trigger>
				{#snippet child({ props })}
					<button
						{...props}
						type="button"
						title={m.recall_title({ date: dayHeading(note.date) })}
						class="flex min-w-0 cursor-pointer items-center gap-1 text-left transition-colors hover:text-neutral-200"
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
	</span>
{:else}
	{@render children?.()}
{/if}
