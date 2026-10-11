<script lang="ts">
	import { tick } from 'svelte';
	import type { Note } from '#lib/api.js';
	import { agendaDay, aheadLabel, dayHeading, shortDay } from '#lib/helpers/dates.js';
	import { noteTitle } from '#lib/markdown.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		notes,
		today,
		class: className = ''
	}: {
		/** The notes that look forward to the week to come, by the day they look to (SPEC 5.3). */
		notes: Note[];
		today: string;
		class?: string;
	} = $props();

	const shell = getShell();
	let section = $state<HTMLElement>();
	/** The note whose action the last press began on. */
	let pressed: string | null = null;

	/** The notes under the day they look forward to, in the order they came. */
	const days = $derived.by(() => {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- made anew from the notes each time
		const byDay = new Map<string, Note[]>();
		for (const note of notes) {
			if (!note.on) continue;
			const day = byDay.get(note.on);
			if (day) day.push(note);
			else byDay.set(note.on, [note]);
		}
		return [...byDay];
	});

	/**
	 * Clear a day read wrong, as the day page's lists do: a press counts only
	 * when it began on the action, and by key the next row takes the focus.
	 */
	async function clear(event: MouseEvent, note: Note, index: number) {
		const byKey = event.detail === 0;
		if (!byKey && pressed !== note.id) {
			void shell.openCited(note);
			return;
		}
		await shell.clearDayAhead(note);
		if (!byKey) return;
		await tick();
		const rows = section?.querySelectorAll<HTMLElement>('li > button:first-child');
		rows?.[Math.min(index, rows.length - 1)]?.focus();
	}
</script>

<!-- The week to come as an agenda: each day under a heading on a hairline,
     its notes under it, each by the day it was written. -->
<section bind:this={section} aria-label={m.calendar_coming_up()} class={className}>
	{#each days as [on, dayNotes] (on)}
		<h2
			class="my-2 flex items-center gap-3 text-xs font-medium tracking-wide text-meta uppercase before:h-px before:flex-1 before:bg-border after:h-px after:flex-1 after:bg-border"
		>
			{agendaDay(on, today)}
		</h2>
		<ul class="mb-4">
			{#each dayNotes as note (note.id)}
				<li class="group/row relative">
					<button
						type="button"
						onclick={() => void shell.openCited(note)}
						onpointerdown={() => (pressed = null)}
						title={m.day_written_on({ date: dayHeading(note.date) })}
						class="group -mx-2 grid w-[calc(100%+1rem)] cursor-pointer grid-cols-[auto_1fr_auto] items-start gap-x-3 rounded-lg px-2 py-2 text-left focus-ring outline-none"
					>
						<!-- A hollow mark, as the day's cell has, which lights in the
						     accent under the pointer, as a note's mark does. -->
						<span aria-hidden="true" class="flex h-5 items-center">
							<span
								class="size-2.5 rounded-full border border-neutral-500 transition-colors group-hover:border-primary group-focus-visible:border-primary"
							></span>
						</span>
						<span class="line-clamp-2 text-sm text-neutral-300">{noteTitle(note)}</span>
						<span
							class="text-xs leading-5 text-meta tabular-nums transition-opacity group-hover/row:opacity-0 group-has-focus-visible/row:opacity-0"
						>
							{shortDay(note.date)}
						</span>
					</button>
					<!-- Shown on hover and on keyboard focus over the day it was
					     written, out of reach while hidden, as on the day page. -->
					<button
						type="button"
						onclick={(event) => void clear(event, note, notes.indexOf(note))}
						onpointerdown={() => (pressed = note.id)}
						class="pointer-events-none absolute top-1.5 right-0 cursor-pointer rounded bg-background px-1.5 py-0.5 text-xs text-neutral-400 opacity-0 focus-ring transition-opacity outline-none group-hover/row:pointer-events-auto group-hover/row:opacity-100 group-has-focus-visible/row:pointer-events-auto group-has-focus-visible/row:opacity-100 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200"
					>
						{m.day_ahead_clear({ date: aheadLabel(on) })}
					</button>
				</li>
			{/each}
		</ul>
	{/each}
</section>
