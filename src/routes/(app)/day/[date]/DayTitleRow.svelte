<script lang="ts">
	import { untrack } from 'svelte';
	import { fade } from 'svelte/transition';
	import { resolve } from '$app/paths';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import { Button } from '#lib/components/ui/button/index.js';
	import { dayHeading } from '#lib/helpers/dates.js';
	import { m } from '#lib/paraglide/messages.js';
	import { mac } from '#lib/helpers/platform.js';
	import DayName from './DayName.svelte';
	import { moveLess, settle } from '#lib/helpers/motion.js';

	let {
		date,
		previous,
		next,
		compact
	}: {
		date: string;
		/** The days the arrows go to; an arrow without one is disabled. */
		previous: string | undefined;
		next: string | undefined;
		/** As the app's header shows it, once the view's heading has scrolled away. */
		compact: boolean;
	} = $props();

	const dayHref = (day: string) => resolve(`day/${day}/`);

	/** The day the title last named. */
	let last = untrack(() => date);

	/** Another day's name comes in from the side it lies on, a later day
	 *  from the right, as the arrows and swipes go. With less motion, a fade. */
	function turn(node: Element) {
		// Leaving for another view, the title is redrawn once without a day.
		if (!date) return { duration: 0 };
		const side = Math.sign(date.localeCompare(last));
		last = date;
		if (moveLess() || side === 0) return fade(node, { duration: 150 });
		return {
			duration: 250,
			easing: settle,
			css: (t: number, u: number) => `opacity: ${t}; transform: translateX(${u * side * 12}px)`
		};
	}
</script>

<!-- The day's title row: the arrows to the days on either side, then its
     name, which opens the calendar. The arrows sit side by side ahead of
     the title, so they stay put while its width changes from one day to
     the next. On a screen too narrow for the date on one line beside them,
     the title goes under them. -->
<div class={compact ? 'contents' : 'flex min-w-0 flex-1 flex-wrap items-center gap-2'}>
	<Button
		variant="ghost"
		size="icon-sm"
		href={previous && dayHref(previous)}
		disabled={!previous}
		aria-label={m.calendar_previous_day()}
		aria-keyshortcuts="Alt+ArrowLeft"
		title="{m.calendar_previous_day()} ({mac ? '⌥←' : 'Alt+←'})"
		class="text-muted-foreground hover:text-foreground [&_svg]:transition-[translate] [&_svg]:duration-200 [&_svg]:ease-settle hover:[&_svg]:-translate-x-0.5"
	>
		<ChevronLeftIcon />
	</Button>
	<Button
		variant="ghost"
		size="icon-sm"
		href={next && dayHref(next)}
		disabled={!next}
		aria-label={m.calendar_next_day()}
		aria-keyshortcuts="Alt+ArrowRight"
		title="{m.calendar_next_day()} ({mac ? '⌥→' : 'Alt+→'})"
		class="text-muted-foreground hover:text-foreground [&_svg]:transition-[translate] [&_svg]:duration-200 [&_svg]:ease-settle hover:[&_svg]:translate-x-0.5"
	>
		<ChevronRightIcon />
	</Button>
	<!-- The date opens the calendar, on the day's month; the calendar after
	     it says it can be clicked. -->
	<svelte:element
		this={compact ? 'span' : 'h1'}
		class={compact
			? 'min-w-0 text-sm font-semibold whitespace-nowrap'
			: 'grow basis-0 text-2xl font-medium max-sm:text-xl'}
	>
		<a
			href={resolve('calendar/')}
			title={m.calendar_pick()}
			class="group/date -mx-1.5 inline-flex max-w-full items-center gap-1.5 rounded-md px-1.5 focus-ring transition-colors hover:bg-muted"
		>
			{#key date}
				<span class={compact ? 'min-w-0 truncate' : undefined} in:turn>
					{#if compact}{dayHeading(date, true)}{:else}<DayName {date} />{/if}
				</span>
			{/key}
			<CalendarIcon
				class={[
					'shrink-0 text-muted-foreground transition-colors group-hover/date:text-foreground',
					compact ? 'size-3.5' : 'size-4.5'
				]}
			/>
		</a>
	</svelte:element>
</div>
