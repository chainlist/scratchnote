<script lang="ts">
	import { untrack } from 'svelte';
	import { fade } from 'svelte/transition';
	import ComingUp from './ComingUp.svelte';
	import DayPreview from './DayPreview.svelte';
	import MonthCalendar from './MonthCalendar.svelte';
	import { notesBetween, type Note } from '#lib/api.js';
	import View from '#lib/components/layout/View.svelte';
	import { plusDays } from '#lib/helpers/dates.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();
	const ahead = $derived(Map.groupBy(data.ahead, (note) => note.on ?? ''));
	/** The month shown, as `2026-10`: where the calendar opens, then where its arrows take it. */
	let month = $state(untrack(() => (shell.day || data.today).slice(0, 7)));
	/** The notes written on the days the month shows, its edges included. */
	let written = $state<Note[]>([]);
	const byDay = $derived(Map.groupBy(written, (note) => note.date));
	/** The day under the pointer or the focus, shown in the agenda's place. */
	let previewed = $state<string | null>(null);
	/** The day last previewed, kept as the week fades back over it. */
	let held = $state(untrack(() => shell.day || data.today));
	/** Whether the column beside the month shows a day rather than the week. */
	const showDay = $derived(previewed !== null || !data.coming.length);
	/**
	 * The day the column shows: the one previewed; with a week to show, the
	 * one last previewed, fading out; else the one chosen, while the month
	 * shown holds it (the notes read are only the month's), else the month's
	 * first.
	 */
	const shownDay = $derived.by(() => {
		if (previewed) return previewed;
		if (data.coming.length) return held;
		const chosen = shell.day || data.today;
		return chosen.startsWith(month) ? chosen : `${month}-01`;
	});
	let wait: ReturnType<typeof setTimeout> | undefined;

	// Read again for another month, and when the notes change (`data` then
	// comes anew). The days of the months either side the grid shows are a
	// week at most each way.
	$effect(() => {
		const first = `${month}-01`;
		void data.days;
		let stale = false;
		notesBetween(plusDays(first, -7), plusDays(first, 31 + 7)).then(
			(notes) => {
				if (!stale) written = notes;
			},
			(e) => shell.fail(m.error_load_view(), e)
		);
		return () => {
			stale = true;
		};
	});

	$effect(() => () => clearTimeout(wait));

	/**
	 * A day shows once the pointer rests on it a moment, so one crossing the
	 * month does not flash the column; from one day to the next it follows
	 * at once, and leaving the days brings the agenda back.
	 */
	function preview(date: string | null) {
		clearTimeout(wait);
		if (date === null || previewed) show(date);
		else wait = setTimeout(() => show(date), 150);
	}

	function show(date: string | null) {
		previewed = date;
		if (date) held = date;
	}
</script>

<View back={shell.back} title={m.calendar_title()} wide>
	<!-- The week to come beside the month on a wide window, under it on a
	     narrow one: the day page keeps to the day, so what lies ahead is
	     looked at here (SPEC 5.3). The month keeps its size without it. Side
	     by side, a day under the pointer takes the week's place while it is
	     there, and with no week to show the chosen day stands in it, so the
	     column is never empty. The two share one cell and cross-fade. Under
	     the month the days are not shown, where they would push the page
	     down. -->
	<div class="@container">
		<div class="grid gap-x-10 gap-y-6 @2xl:grid-cols-[minmax(0,1fr)_20rem]">
			<MonthCalendar
				days={data.days}
				written={byDay}
				{ahead}
				selected={shell.day}
				onselect={(date) => void shell.openDay(date)}
				onmonth={(shown) => (month = shown)}
				onpreview={preview}
			/>
			<div class="grid content-start @2xl:pt-1">
				<!-- One day fades into the next as the pointer moves along. -->
				<div
					class="hidden transition-[opacity,visibility] duration-200 ease-out [grid-area:1/1] @2xl:grid {showDay
						? ''
						: 'invisible opacity-0'}"
				>
					{#key shownDay}
						<div class="[grid-area:1/1]" transition:fade={{ duration: 150 }}>
							<DayPreview
								date={shownDay}
								written={byDay.get(shownDay) ?? []}
								ahead={ahead.get(shownDay) ?? []}
								today={data.today}
							/>
						</div>
					{/key}
				</div>
				{#if data.coming.length}
					<ComingUp
						notes={data.coming}
						today={data.today}
						class="transition-[opacity,visibility] duration-200 ease-out [grid-area:1/1] {showDay
							? '@2xl:invisible @2xl:opacity-0'
							: ''}"
					/>
				{/if}
			</div>
		</div>
	</div>
</View>
