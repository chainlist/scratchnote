<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import { isPage, type Note } from '#lib/api.js';
	import NoteCard from '#lib/components/NoteCard.svelte';
	import PageCard from '#lib/components/PageCard.svelte';
	import { Checkbox } from '#lib/components/ui/checkbox/index.js';
	import { m } from '#lib/paraglide/messages.js';

	type Actions = Pick<
		ComponentProps<typeof NoteCard>,
		'onedit' | 'ondelete' | 'onsave' | 'onsimilar' | 'onpage' | 'onmove' | 'onerror'
	> &
		Pick<ComponentProps<typeof PageCard>, 'onopen' | 'ondock'>;

	let {
		notes,
		empty,
		showDate = false,
		blinking = null,
		threadLine = true,
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
		/** The notes chosen, while notes are being chosen: each card then has a box. */
		chosen?: string[] | null;
		onchoose?: (id: string, on: boolean) => void;
	} = $props();
</script>

{#if notes.length === 0}
	<p class="text-base text-neutral-600">{empty}</p>
{:else}
	<ul>
		{#each notes as note (note.id)}
			<li class={chosen ? 'flex items-start gap-3' : undefined}>
				{#if chosen}
					<Checkbox
						checked={chosen.includes(note.id)}
						onCheckedChange={(on) => onchoose?.(note.id, on)}
						aria-label={m.thread_select()}
						class="mt-6.5"
					/>
				{/if}
				{#if isPage(note)}
					<PageCard
						{note}
						onopen={actions.onopen}
						ondock={actions.ondock}
						ondelete={actions.ondelete}
						{showDate}
						{threadLine}
						blink={note.id === blinking}
					/>
				{:else}
					<NoteCard {note} {...actions} {showDate} {threadLine} blink={note.id === blinking} />
				{/if}
			</li>
		{/each}
	</ul>
{/if}
