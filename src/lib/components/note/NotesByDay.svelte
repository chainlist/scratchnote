<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import type { Note } from '#lib/api.js';
	import NoteList from '#lib/components/note/NoteList.svelte';
	import ShowMore from '#lib/components/ShowMore.svelte';
	import { dayHeading, monthHeading, weekday } from '#lib/dates.js';
	import { m } from '#lib/paraglide/messages.js';
	import { RESULTS_STEP } from '#lib/query.js';

	let {
		notes,
		...list
	}: Omit<ComponentProps<typeof NoteList>, 'notes' | 'empty' | 'showDate'> & {
		/** In the order they are shown, a day's notes side by side. */
		notes: Note[];
	} = $props();

	/** How many notes are drawn: a stretch at first, as many more on asking,
	 *  so a name or a thread of thousands opens at once (each card costs
	 *  about a millisecond to draw). */
	let drawn = $state(RESULTS_STEP);
	/** As far as the note brought into sight, when it lies past those drawn,
	 *  as a thread's arc or a task still open leads to. */
	const reach = $derived(
		list.blinking ? notes.findIndex((note) => note.id === list.blinking) + 1 : 0
	);
	// And kept drawn once the note stops blinking.
	$effect(() => {
		if (reach > drawn) drawn = Math.ceil(reach / RESULTS_STEP) * RESULTS_STEP;
	});
	const count = $derived(Math.max(drawn, reach));

	/** The notes by month, then by day, in the order they come; a month
	 *  counts all of its notes, drawn or not yet. */
	const months = $derived.by(() => {
		const months: { month: string; count: number; days: { date: string; notes: Note[] }[] }[] = [];
		for (const [i, note] of notes.entries()) {
			const month = note.date.slice(0, 7);
			if (months.at(-1)?.month !== month) months.push({ month, count: 0, days: [] });
			const into = months.at(-1)!;
			into.count++;
			if (i >= count) continue;
			const day = into.days.at(-1);
			if (day?.date === note.date) day.notes.push(note);
			else into.days.push({ date: note.date, notes: [note] });
		}
		return months.filter((month) => month.days.length);
	});
</script>

<!-- Notes across days, as a calendar reads: each month under its name with
     how many notes it holds, each day in a column of its own beside its
     notes, its number large and its weekday under it, kept in view while
     its notes scroll by. The column lines up with the first note's time.
     A faint line parts the days of a month. -->
{#each months as { month, count, days } (month)}
	<section class="mb-10 last:mb-0">
		<h2 class="mb-1 flex items-baseline gap-2">
			<span class="text-base font-semibold text-neutral-200">{monthHeading(month)}</span>
			<span class="text-xs text-meta tabular-nums">{m.tags_notes({ count })}</span>
		</h2>
		{#each days as day (day.date)}
			<section
				class="grid grid-cols-[2.75rem_minmax(0,1fr)] gap-x-4 border-neutral-800/70 [&+&]:border-t"
			>
				<h3 class="sticky top-0 self-start pt-5 text-center">
					<span class="sr-only">{dayHeading(day.date)}</span>
					<span aria-hidden="true" class="flex flex-col items-center">
						<span class="text-2xl leading-6 font-semibold text-neutral-100 tabular-nums">
							{Number(day.date.slice(8))}
						</span>
						<span class="mt-1 text-xs text-meta">{weekday(day.date)}</span>
					</span>
				</h3>
				<NoteList notes={day.notes} empty="" {...list} />
			</section>
		{/each}
	</section>
{/each}
{#if count < notes.length}
	<ShowMore
		class="mt-8"
		count={notes.length - count}
		onclick={() => (drawn = count + RESULTS_STEP)}
	/>
{/if}
