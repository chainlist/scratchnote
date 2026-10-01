<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import { isPage, type Note } from '$lib/api';
	import NoteCard from '$lib/components/NoteCard.svelte';
	import PageCard from '$lib/components/PageCard.svelte';

	type Actions = Pick<
		ComponentProps<typeof NoteCard>,
		| 'onedit'
		| 'ondelete'
		| 'onsave'
		| 'onretry'
		| 'oncategory'
		| 'onsimilar'
		| 'onpage'
		| 'onmove'
		| 'onerror'
	> &
		Pick<ComponentProps<typeof PageCard>, 'onopen' | 'ondock'>;

	let {
		notes,
		empty,
		showDate = false,
		blinking = null,
		...actions
	}: Actions & {
		notes: Note[];
		/** Shown in place of the list when there is nothing in it. */
		empty: string;
		/** Set when the list spans days, so each card says which one. */
		showDate?: boolean;
		/** The note to blink once, where a chat citation led. */
		blinking?: string | null;
	} = $props();
</script>

{#if notes.length === 0}
	<p class="text-base text-neutral-600">{empty}</p>
{:else}
	<ul>
		{#each notes as note (note.id)}
			<li>
				{#if isPage(note)}
					<PageCard
						{note}
						onopen={actions.onopen}
						ondock={actions.ondock}
						ondelete={actions.ondelete}
						oncategory={actions.oncategory}
						{showDate}
						blink={note.id === blinking}
					/>
				{:else}
					<NoteCard {note} {...actions} {showDate} blink={note.id === blinking} />
				{/if}
			</li>
		{/each}
	</ul>
{/if}
