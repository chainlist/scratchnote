<script lang="ts" module>
	import { getLocale } from '$lib/paraglide/runtime';

	/** A day ahead in a few characters, its weekday first: the year only when it is not this one. */
	export function aheadLabel(date: string) {
		const day = new Date(`${date}T00:00:00`);
		const thisYear = day.getFullYear() === new Date().getFullYear();
		return day.toLocaleDateString(getLocale(), {
			weekday: 'short',
			day: 'numeric',
			month: 'short',
			...(thisYear ? {} : { year: 'numeric' })
		});
	}
</script>

<script lang="ts">
	import { dayHeading } from '$lib/components/ViewHeader.svelte';
	import CalendarClockIcon from '@lucide/svelte/icons/calendar-clock';
	import { m } from '$lib/paraglide/messages';
	import { getShell } from '$lib/shell.svelte';
	import { cn } from '$lib/utils.js';

	let {
		on,
		class: className
	}: {
		/** The later day the note looks forward to (SPEC 5.7). */
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
		'flex shrink-0 cursor-pointer items-center gap-1.5 text-xs text-neutral-500 transition-colors hover:text-neutral-200',
		className
	)}
>
	<CalendarClockIcon class="size-3 shrink-0" />{aheadLabel(on)}
</button>
