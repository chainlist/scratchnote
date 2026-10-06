<script lang="ts">
	import type { ComponentProps } from 'svelte';
	import type { Note } from '#lib/api.js';
	import NoteList from '#lib/components/NoteList.svelte';
	import { dayHeading } from '#lib/components/ViewHeader.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';

	let {
		notes,
		...list
	}: Omit<ComponentProps<typeof NoteList>, 'notes' | 'empty' | 'showDate'> & {
		/** In the order they are shown, a day's notes side by side. */
		notes: Note[];
	} = $props();

	/** The notes by month, then by day, in the order they come. */
	const months = $derived.by(() => {
		const months: { month: string; count: number; days: { date: string; notes: Note[] }[] }[] = [];
		for (const note of notes) {
			const month = note.date.slice(0, 7);
			if (months.at(-1)?.month !== month) months.push({ month, count: 0, days: [] });
			const into = months.at(-1)!;
			into.count++;
			const day = into.days.at(-1);
			if (day?.date === note.date) day.notes.push(note);
			else into.days.push({ date: note.date, notes: [note] });
		}
		return months;
	});

	/** A month as its heading reads, its year left out in this one. */
	function monthHeading(month: string) {
		const day = new Date(`${month}-01T00:00:00`);
		const thisYear = day.getFullYear() === new Date().getFullYear();
		const name = new Intl.DateTimeFormat(getLocale(), {
			month: 'long',
			...(thisYear ? {} : { year: 'numeric' })
		}).format(day);
		return name.charAt(0).toLocaleUpperCase(getLocale()) + name.slice(1);
	}

	const weekday = (date: string) =>
		new Intl.DateTimeFormat(getLocale(), { weekday: 'short' }).format(new Date(`${date}T00:00:00`));
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
			<span class="text-xs text-neutral-500 tabular-nums">{m.tags_notes({ count })}</span>
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
						<span class="mt-1 text-xs text-neutral-500">{weekday(day.date)}</span>
					</span>
				</h3>
				<NoteList notes={day.notes} empty="" {...list} />
			</section>
		{/each}
	</section>
{/each}
