<script lang="ts">
	import { untrack } from 'svelte';
	import { getLocalTimeZone, isToday, parseDate, type DateValue } from '@internationalized/date';
	import { Calendar as CalendarPrimitive } from 'bits-ui';
	import type { DaySummary, Note } from '#lib/api.js';
	import * as Calendar from '#lib/components/ui/calendar/index.js';
	import { noteTitle } from '#lib/markdown.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';

	let {
		days,
		written,
		ahead,
		selected,
		onselect,
		onmonth,
		onpreview
	}: {
		days: DaySummary[];
		/** The notes written on the days shown, by day, named in the cells of a wide calendar. */
		written: Map<string, Note[]>;
		/** The notes that look forward to the days ahead, by that day (SPEC 5.3). */
		ahead: Map<string, Note[]>;
		/** The day shown before, whose month the calendar opens on. */
		selected: string;
		onselect: (date: string) => void;
		/** The month shown, as `2026-10`, each time another is. */
		onmonth: (month: string) => void;
		/** The day under the pointer or the focus, and `null` once neither is on a day. */
		onpreview: (date: string | null) => void;
	} = $props();

	/** How many lines a wide cell names, the last a count of the rest when there are more. */
	const LINES = 4;

	const locale = $derived(getLocale());
	const counts = $derived(new Map(days.map((day) => [day.date, day.count])));
	const top = $derived(Math.max(1, ...days.map((day) => day.count)));
	// Only where the calendar opens; its own arrows move it from there.
	let placeholder = $state<DateValue | undefined>(
		untrack(() => (selected ? parseDate(selected) : undefined))
	);

	/**
	 * How many dots mark a day, on a log scale so one busy day does not
	 * wash out the rest: one to three, none without notes.
	 */
	function dots(count: number) {
		if (count === 0) return 0;
		const weight = top > 1 ? Math.log(count) / Math.log(top) : 1;
		return weight > 0.66 ? 3 : weight > 0.33 ? 2 : 1;
	}

	function pick(date: DateValue | undefined) {
		if (date) onselect(date.toString());
	}

	/** A pointer on a day shows it, but not a finger: a tap opens the day. */
	function hover(event: PointerEvent, date: DateValue) {
		if (event.pointerType !== 'touch') onpreview(date.toString());
	}

	/** The focus leaving the days, not moving between them. */
	function leave(event: FocusEvent) {
		const to = event.relatedTarget;
		if (!(to instanceof Node && (event.currentTarget as Node).contains(to))) onpreview(null);
	}
</script>

<!-- Composed from the calendar's parts rather than its wrapper, so the day
     cells can fill the page while the arrows and heading keep their size. -->
<CalendarPrimitive.Root
	type="single"
	bind:placeholder
	onValueChange={pick}
	onPlaceholderChange={(date) => onmonth(date.toString().slice(0, 7))}
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
					<Calendar.Grid onpointerleave={() => onpreview(null)} onfocusout={leave}>
						<Calendar.GridHead>
							<Calendar.GridRow class="select-none">
								{#each weekdays as weekday, i (i)}
									<Calendar.HeadCell
										class="w-[calc(100cqw/7)] text-xs font-medium tracking-wide uppercase"
										>{weekday}</Calendar.HeadCell
									>
								{/each}
							</Calendar.GridRow>
						</Calendar.GridHead>
						<Calendar.GridBody>
							{#each month.weeks as weekDates, row (weekDates)}
								<Calendar.GridRow class="mt-1 w-full">
									{#each weekDates as date, col (date)}
										{@const key = date.toString()}
										{@const count = counts.get(key) ?? 0}
										{@const lines = [...(written.get(key) ?? []), ...(ahead.get(key) ?? [])]}
										{@const named = lines.length > LINES ? lines.slice(0, LINES - 1) : lines}
										<Calendar.Cell
											{date}
											month={month.value}
											class="h-12 w-[calc(100cqw/7)] p-0.5 @2xl:h-28"
										>
											<!-- No fill for today: the circle round its number marks it. -->
											<Calendar.Day
												style="--day-wave: {row + col}"
												title={count > 0 ? m.tags_notes({ count }) : undefined}
												onpointerenter={(event: PointerEvent) => hover(event, date)}
												onfocus={() => onpreview(key)}
												class="calendar-day-in size-full gap-1 @2xl:items-stretch @2xl:justify-start @2xl:bg-primary/4 @2xl:p-1.5 @2xl:[&[data-today]:not([data-selected])]:bg-primary/4 [&[data-today]:not([data-selected])]:bg-transparent"
											>
												{#snippet children({ day: number })}
													<span
														class="flex size-7 shrink-0 items-center justify-center rounded-full text-sm! opacity-100! @2xl:size-6 @2xl:self-start {isToday(
															date,
															getLocalTimeZone()
														)
															? 'bg-primary font-medium text-primary-foreground'
															: ''}">{number}</span
													>
													<!-- Under the number, a dot or up to three for the notes
													     written, more on a busier day, and a hollow one for
													     a day a note looks forward to, as a note's mark on the
													     timeline. Quieter on a day outside the month. -->
													<span
														aria-hidden="true"
														class="flex h-1.5 items-center gap-0.5 opacity-100! @2xl:hidden [[data-outside-month]_&]:opacity-50!"
													>
														{#each { length: dots(count) }, i (i)}
															<span class="size-1.5 rounded-full bg-primary"></span>
														{/each}
														{#if ahead.has(key)}
															<span class="size-1.5 rounded-full border border-neutral-500"></span>
														{/if}
													</span>
													<!-- Wide, the day names its notes instead, a line each,
													     and a note looking forward to it after its hollow
													     mark, in the quiet tone. -->
													<div
														aria-hidden="true"
														class="hidden w-full min-w-0 flex-col gap-px text-left text-[0.7rem] leading-4 @2xl:flex [[data-outside-month]_&]:opacity-50"
													>
														{#each named as note (note.id)}
															{#if note.date === key}
																<span class="truncate text-neutral-300">{noteTitle(note)}</span>
															{:else}
																<span class="flex min-w-0 items-center gap-1 text-meta">
																	<span
																		class="size-1.5 shrink-0 rounded-full border border-neutral-500"
																	></span>
																	<span class="truncate">{noteTitle(note)}</span>
																</span>
															{/if}
														{/each}
														{#if named.length < lines.length}
															<span class="text-meta">+{lines.length - named.length}</span>
														{/if}
													</div>
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
