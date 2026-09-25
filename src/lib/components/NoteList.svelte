<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import type { Note } from '$lib/api';
	import NoteCard from '$lib/components/NoteCard.svelte';

	type Actions = Pick<
		ComponentProps<typeof NoteCard>,
		'onedit' | 'ondelete' | 'onsave' | 'onretry' | 'ontag' | 'onsimilar'
	>;

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
				<NoteCard {note} {...actions} {showDate} blink={note.id === blinking} />
			</li>
		{/each}
	</ul>
{/if}
