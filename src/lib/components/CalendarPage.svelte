<script lang="ts">
	import { untrack } from 'svelte';
	import {
		getLocalTimeZone,
		isEqualMonth,
		isToday,
		parseDate,
		type DateValue
	} from '@internationalized/date';
	import { Calendar as CalendarPrimitive } from 'bits-ui';
	import type { DaySummary } from '$lib/api';
	import * as Calendar from '$lib/components/ui/calendar';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';

	let {
		days,
		selected,
		onselect
	}: {
		days: DaySummary[];
		/** The day shown before, whose month the calendar opens on. */
		selected: string;
		onselect: (date: string) => void;
	} = $props();

	const locale = $derived(getLocale());
	const counts = $derived(new Map(days.map((day) => [day.date, day.count])));
	const top = $derived(Math.max(1, ...days.map((day) => day.count)));
	// Only where the calendar opens; its own arrows move it from there.
	let placeholder = $state<DateValue | undefined>(
		untrack(() => (selected ? parseDate(selected) : undefined))
	);

	/**
	 * How strongly a day is shaded, on a log scale so one busy day does not
	 * wash out the rest. Today's selector is repeated so its shade replaces
	 * the accent fill the day would otherwise get. A day without notes keeps
	 * a very faint tint, so the grid still reads as cells.
	 */
	function shade(count: number) {
		if (count === 0) return 'bg-primary/4 [&[data-today]:not([data-selected])]:bg-primary/4';
		const weight = top > 1 ? Math.log(count) / Math.log(top) : 1;
		if (weight > 0.66) return 'bg-primary/25 [&[data-today]:not([data-selected])]:bg-primary/25';
		if (weight > 0.33) return 'bg-primary/15 [&[data-today]:not([data-selected])]:bg-primary/15';
		return 'bg-primary/8 [&[data-today]:not([data-selected])]:bg-primary/8';
	}

	function pick(date: DateValue | undefined) {
		if (date) onselect(date.toString());
	}
</script>

<!-- Composed from the calendar's parts rather than its wrapper, so the day
     cells can fill the page while the arrows and heading keep their size. -->
<CalendarPrimitive.Root
	type="single"
	bind:placeholder
	onValueChange={pick}
	{locale}
	weekdayFormat="short"
	disableDaysOutsideMonth={false}
	class="group/calendar @container [--cell-radius:var(--radius-lg)] [--cell-size:--spacing(8)]"
>
	{#snippet children({ months, weekdays })}
		<Calendar.Months>
			<Calendar.Nav>
				<Calendar.PrevButton variant="ghost" />
				<Calendar.NextButton variant="ghost" />
			</Calendar.Nav>
			{#each months as month (month)}
				<Calendar.Month>
					<Calendar.Header>
						<Calendar.Heading class="text-base" />
					</Calendar.Header>
					<Calendar.Grid>
						<Calendar.GridHead>
							<Calendar.GridRow class="select-none">
								{#each weekdays as weekday, i (i)}
									<Calendar.HeadCell class="w-[calc(100cqw/7)]">{weekday}</Calendar.HeadCell>
								{/each}
							</Calendar.GridRow>
						</Calendar.GridHead>
						<Calendar.GridBody>
							{#each month.weeks as weekDates, row (weekDates)}
								<Calendar.GridRow class="mt-1 w-full">
									{#each weekDates as date, col (date)}
										{@const count = counts.get(date.toString()) ?? 0}
										<Calendar.Cell {date} month={month.value} class="h-20 w-[calc(100cqw/7)] p-0.5">
											<Calendar.Day
												style="--day-wave: {row + col}"
												class="calendar-day-in size-full items-start justify-between p-2 {shade(
													isEqualMonth(date, month.value) ? count : 0
												)}"
											>
												{#snippet children({ day: number })}
													<!-- Today is marked by a circle round its number rather than a filled cell. -->
													<span
														class="-m-1 flex size-6 items-center justify-center rounded-full text-sm! opacity-100! {isToday(
															date,
															getLocalTimeZone()
														)
															? 'bg-primary font-medium text-primary-foreground'
															: ''}">{number}</span
													>
													{#if count > 0}
														<span>{m.tags_notes({ count })}</span>
													{/if}
												{/snippet}
											</Calendar.Day>
										</Calendar.Cell>
									{/each}
								</Calendar.GridRow>
							{/each}
						</Calendar.GridBody>
					</Calendar.Grid>
				</Calendar.Month>
			{/each}
		</Calendar.Months>
	{/snippet}
</CalendarPrimitive.Root>
