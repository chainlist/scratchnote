<script lang="ts">
	import type { Note } from '#lib/api.js';
	import NoteList from '#lib/components/NoteList.svelte';
	import View from '#lib/components/View.svelte';
	import { dayHeading, shortDay } from '#lib/components/ViewHeader.svelte';
	import { mentionKey } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data, params } = $props();

	const shell = getShell();
	/** As the newest note types it, or as the link has it once none does. */
	const name = $derived(data.found.name ?? params.name);

	/** The notes by day, newest first, as they come. */
	const days = $derived.by(() => {
		const days: { date: string; notes: Note[] }[] = [];
		for (const note of data.found.notes) {
			const last = days.at(-1);
			if (last?.date === note.date) last.notes.push(note);
			else days.push({ date: note.date, notes: [note] });
		}
		return days;
	});
</script>

<!-- The notes and pages that mention a name, each day under its own heading
     (SPEC 3.10). Keyed by the name, so another one rises into place. -->
<View
	back={shell.back}
	title={`@${name}`}
	detail={data.found.notes.length
		? m.thread_detail({
				count: data.found.notes.length,
				date: shortDay(data.found.notes.at(-1)!.date)
			})
		: undefined}
	pin={{ kind: 'mention', target: mentionKey(name), label: name }}
	key={mentionKey(params.name)}
>
	{#each days as day (day.date)}
		<section class="mb-8 last:mb-0">
			<h2 class="border-b border-neutral-800 pb-2 text-sm font-medium text-neutral-400">
				{dayHeading(day.date)}
			</h2>
			<NoteList notes={day.notes} empty="" {...shell.cardActions} />
		</section>
	{:else}
		<p class="text-base text-neutral-600">{m.mentions_nothing({ name: `@${name}` })}</p>
	{/each}
</View>
