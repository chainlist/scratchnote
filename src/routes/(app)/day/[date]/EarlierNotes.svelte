<script lang="ts">
	import CalendarClockIcon from '@lucide/svelte/icons/calendar-clock';
	import type { Note } from '#lib/api.js';
	import { dayHeading, shortDay } from '#lib/dates.js';
	import { noteTitle } from '#lib/markdown.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { notes }: { notes: Note[] } = $props();

	const shell = getShell();
</script>

<!-- What earlier notes said about this day (SPEC 5.3), on the day's own
     columns, the day written where a note's time is, so it reads as part of
     the page rather than a box laid on it. -->
<section class="mb-8">
	<h2 class="mb-1 flex items-center gap-1.5 pl-24 text-xs font-medium text-meta">
		<CalendarClockIcon class="size-3.5" />{m.day_from_earlier()}
	</h2>
	<ul>
		{#each notes as note (note.id)}
			<li>
				<button
					type="button"
					onclick={() => void shell.openCited(note)}
					title={m.day_written_on({ date: dayHeading(note.date) })}
					class="group -mx-3 grid w-[calc(100%+1.5rem)] cursor-pointer grid-cols-[4.5rem_1fr] items-baseline gap-x-6 rounded-lg px-3 py-1.5 text-left focus-ring outline-none"
				>
					<!-- The day it was written, which brightens with a tick of the
					     accent, as a note's time does. -->
					<span
						class="relative text-right text-xs text-meta tabular-nums transition-colors group-hover:text-neutral-300 group-focus-visible:text-neutral-300"
					>
						{shortDay(note.date)}
						<span
							aria-hidden="true"
							class="absolute top-1/2 -right-3 h-3 w-0.5 -translate-y-1/2 rounded-full bg-primary opacity-0 transition-opacity group-hover:opacity-100 group-focus-visible:opacity-100"
						></span>
					</span>
					<span class="truncate text-sm text-neutral-300">{noteTitle(note)}</span>
				</button>
			</li>
		{/each}
	</ul>
</section>
