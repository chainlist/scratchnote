<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import { fade } from 'svelte/transition';
	import { isPage, type Note } from '#lib/api.js';
	import NoteCard from '#lib/components/note/NoteCard.svelte';
	import PageCard from '#lib/components/note/PageCard.svelte';
	import { daySpacing } from '#lib/helpers/dates.js';
	import { noteTitle } from '#lib/markdown.js';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';

	type Actions = Pick<
		ComponentProps<typeof NoteCard>,
		'ondelete' | 'onsave' | 'onsimilar' | 'onpage' | 'onmove' | 'onerror'
	> &
		Pick<ComponentProps<typeof PageCard>, 'onopen' | 'ondock'>;

	let {
		notes,
		empty,
		showDate = false,
		blinking = null,
		threadLine = true,
		partLevel = 2,
		chosen = null,
		onchoose,
		...actions
	}: Actions & {
		notes: Note[];
		/** Shown in place of the list when there is nothing in it. */
		empty: string;
		/** Set when the list spans days, so each card says which one. */
		showDate?: boolean;
		/** The note to blink once, where a link to it led. */
		blinking?: string | null;
		/** Off in a thread's own view, where every card is of the one thread. */
		threadLine?: boolean;
		/** The heading level of the parts of the day: one under the view's own. */
		partLevel?: number;
		/** The notes chosen, while notes are being chosen: each card then has a box. */
		chosen?: string[] | null;
		onchoose?: (id: string, on: boolean) => void;
	} = $props();

	/** One day's notes are spaced by the time between them and named by
	 *  their parts of the day; a list across days keeps its dates instead. */
	const spacing = $derived(showDate ? [] : daySpacing(notes));

	/** More than this many notes coming at once are a list drawn anew or a
	 *  stretch more, which fades in together; a note or two written or
	 *  moved just shows. */
	const FEW = 3;
	/** The notes shown, and how many came when they last changed. */
	let shown: string[] = [];
	let came = 0;
	$effect.pre(() => {
		const ids = notes.map((note) => note.id);
		const before = new Set(shown);
		came = ids.filter((id) => !before.has(id)).length;
		shown = ids;
	});

	function arrive(node: HTMLElement) {
		return came > FEW ? fade(node, { duration: 200 }) : { duration: 0 };
	}
</script>

{#if notes.length === 0}
	<p class="text-base text-meta">{empty}</p>
{:else}
	<ul class={chosen ? 'pl-8' : undefined}>
		{#each notes as note, i (note.id)}
			{@const timeline = { ...spacing[i], partLevel, aside: chosen ? box : undefined }}
			<!-- A chosen note is washed with the accent, so the choice reads while
			     scrolling, not only from its box. -->
			<li class={[chosen?.includes(note.id) && '[&>div>article]:bg-primary/8']} in:arrive>
				{#if isPage(note)}
					<PageCard
						{note}
						onopen={actions.onopen}
						ondock={actions.ondock}
						ondelete={actions.ondelete}
						{showDate}
						{threadLine}
						{timeline}
						blink={note.id === blinking}
					/>
				{:else}
					<NoteCard
						{note}
						{...actions}
						{showDate}
						{threadLine}
						{timeline}
						blink={note.id === blinking}
					/>
				{/if}
			</li>

			<!-- While notes are being chosen, a box left of the margin, level
			     with the note's first line however much room is above it. Round,
			     so it is never taken for one of the note's own square tasks. -->
			{#snippet box()}
				{#if chosen}
					<Checkbox
						checked={chosen.includes(note.id)}
						onCheckedChange={(on) => onchoose?.(note.id, on)}
						aria-label="{note.time}, {noteTitle(note)}"
						class="rounded-full"
					/>
				{/if}
			{/snippet}
		{/each}
	</ul>
{/if}
