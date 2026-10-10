<script lang="ts">
	import { aheadLabel, dayHeading } from '#lib/dates.js';
	import CalendarClockIcon from '@lucide/svelte/icons/calendar-clock';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { cn } from '#lib/utils.js';

	let {
		on,
		class: className
	}: {
		/** The later day the note looks forward to (SPEC 5.3). */
		on: string;
		class?: string;
	} = $props();

	const shell = getShell();
</script>

<!-- The day the note comes back on, which opens that day. -->
<button
	type="button"
	onclick={(event) => {
		// A page's card opens the page on any other click.
		event.stopPropagation();
		void shell.openDay(on);
	}}
	title={m.day_ahead_title({ date: dayHeading(on) })}
	class={cn(
		'flex shrink-0 cursor-pointer items-center gap-1.5 text-xs text-meta transition-colors hover:text-neutral-200',
		className
	)}
>
	<CalendarClockIcon class="size-3 shrink-0" />{aheadLabel(on)}
</button>
