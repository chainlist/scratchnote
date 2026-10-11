<script lang="ts">
	import { tick } from 'svelte';
	import type CalendarClockIcon from '@lucide/svelte/icons/calendar-clock';
	import type { Note } from '#lib/api.js';
	import { aheadLabel, dayHeading } from '#lib/helpers/dates.js';
	import { noteTitle } from '#lib/markdown.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		notes,
		heading,
		icon: Icon,
		day,
		class: className = ''
	}: {
		/** Notes and pages that look forward to a day (SPEC 5.3). */
		notes: Note[];
		heading: string;
		icon: typeof CalendarClockIcon;
		/** What the margin names: the day a note was written, or the day it looks to. */
		day: (note: Note) => string;
		class?: string;
	} = $props();

	const shell = getShell();
	let list = $state<HTMLUListElement>();
	/** The note whose action the last press began on. */
	let pressed: string | null = null;

	/**
	 * Clear a day read wrong. A press counts only when it began on the action:
	 * a finger landing on a row of a touch screen can bring the action up under
	 * itself, and that tap is the row's. A key has no pointer, and the next row
	 * then takes the focus the cleared one held.
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
		const rows = list?.querySelectorAll<HTMLElement>(':scope > li > button:first-child');
		rows?.[Math.min(index, rows.length - 1)]?.focus();
	}
</script>

<!-- Notes that look forward to a day (SPEC 5.3), on the day's own columns,
     a day where a note's time is, so they read as part of the page rather
     than a box laid on it. -->
<section class={className}>
	<h2 class="mb-1 flex items-center gap-1.5 pl-24 text-xs font-medium text-meta">
		<Icon class="size-3.5" />{heading}
	</h2>
	<ul bind:this={list}>
		{#each notes as note, index (note.id)}
			<li class="group/row relative">
				<button
					type="button"
					onclick={() => void shell.openCited(note)}
					onpointerdown={() => (pressed = null)}
					title={m.day_written_on({ date: dayHeading(note.date) })}
					class="group -mx-3 grid w-[calc(100%+1.5rem)] cursor-pointer grid-cols-[4.5rem_1fr] items-baseline gap-x-6 rounded-lg px-3 py-1.5 text-left focus-ring outline-none"
				>
					<!-- The margin's day, which brightens with a tick of the accent,
					     as a note's time does. -->
					<span
						class="relative text-right text-xs text-meta tabular-nums transition-colors group-hover:text-neutral-300 group-focus-visible:text-neutral-300"
					>
						{day(note)}
						<span
							aria-hidden="true"
							class="absolute top-1/2 -right-3 h-3 w-0.5 -translate-y-1/2 rounded-full bg-primary opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
						></span>
					</span>
					<span class="truncate text-sm text-neutral-300">{noteTitle(note)}</span>
				</button>
				{#if note.on}
					{@const on = note.on}
					<!-- A day read wrong, cleared in one click, as the note's menu
					     does. Shown on hover and on keyboard focus, not on the focus
					     a tap gives, and out of reach while hidden, so a tap on a
					     phone cannot clear one unseen. -->
					<button
						type="button"
						onclick={(event) => void clear(event, note, index)}
						onpointerdown={() => (pressed = note.id)}
						class="pointer-events-none absolute top-1/2 right-0 -translate-y-1/2 cursor-pointer rounded bg-background px-1.5 py-0.5 text-xs text-neutral-400 opacity-0 focus-ring transition-opacity outline-none group-hover/row:pointer-events-auto group-hover/row:opacity-100 group-has-focus-visible/row:pointer-events-auto group-has-focus-visible/row:opacity-100 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200"
					>
						{m.day_ahead_clear({ date: aheadLabel(on) })}
					</button>
				{/if}
			</li>
		{/each}
	</ul>
</section>
