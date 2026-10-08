<script lang="ts">
	import type { Note, Thread } from '#lib/api.js';
	import { dayHeading, shortDay } from '#lib/components/ViewHeader.svelte';
	import { daysAgo } from '#lib/days.js';
	import { m } from '#lib/paraglide/messages.js';
	import { threadHue } from '#lib/threads.js';

	let {
		thread,
		notes,
		onday
	}: {
		thread: Thread;
		/** The thread's notes, oldest first. */
		notes: Note[];
		/** A day's dot was picked: bring that day into sight. */
		onday: (date: string) => void;
	} = $props();

	/** Each day the thread holds notes on, with how many, oldest first. */
	const days = $derived.by(() => {
		const count: Record<string, number> = {};
		for (const note of notes) count[note.date] = (count[note.date] ?? 0) + 1;
		return Object.entries(count).map(([date, notes]) => ({ date, notes }));
	});

	/** The lane runs from the thread's first day to today, so a thread
	 *  that went quiet shows it; two weeks at least, so a young one's dots
	 *  do not sit on top of each other. */
	const span = $derived(Math.max(14, daysAgo(thread.since)));
	// The local day as YYYY-MM-DD, as the days of notes are written.
	const local = (date: Date) => date.toLocaleDateString('en-CA');
	const today = local(new Date());
	/** The lane's first day, which its left end names: two weeks back for a
	 *  young thread rather than its own first day, which sits further in. */
	const start = $derived.by(() => {
		const now = new Date();
		return local(new Date(now.getFullYear(), now.getMonth(), now.getDate() - span));
	});
	/** Where a day falls along the lane, in percent from its left. A note
	 *  written on a day ahead sits at today's end. */
	const at = (date: string) => Math.min(100, Math.max(0, (1 - daysAgo(date) / span) * 100));

	/** The lane's width, to tell which days' dots would cover each other. */
	let width = $state(0);
	/** Days closer than this, in pixels, share one dot that counts them all,
	 *  so none is hidden under another's. */
	const NEAR = 12;

	/** The dots: a day each, or a run of days too close to tell apart. */
	const dots = $derived.by(() => {
		const dots: { from: string; to: string; notes: number; at: number }[] = [];
		for (const day of days) {
			const x = at(day.date);
			const last = dots.at(-1);
			// Before the lane is measured, every day keeps a dot of its own.
			if (last && width > 0 && ((x - last.at) / 100) * width < NEAR) {
				last.to = day.date;
				last.notes += day.notes;
			} else dots.push({ from: day.date, to: day.date, notes: day.notes, at: x });
		}
		return dots;
	});

	const label = (dot: (typeof dots)[number]) =>
		`${
			dot.from === dot.to
				? dayHeading(dot.from)
				: m.thread_arc_range({ from: shortDay(dot.from), to: shortDay(dot.to) })
		}, ${m.tags_notes({ count: dot.notes })}`;

	/** A dot with more notes is larger. */
	const size = (notes: number) => (notes >= 4 ? 'size-3' : notes >= 2 ? 'size-2.5' : 'size-2');

	/** The dot Tab lands on, the latest until the arrows move it: the lane is
	 *  one stop, its days one arrow apart, as a toolbar's buttons are. */
	let current = $state<number | null>(null);
	const stop = $derived(Math.min(current ?? dots.length - 1, dots.length - 1));
	const buttons: HTMLButtonElement[] = $state([]);

	function onKeydown(event: KeyboardEvent) {
		const last = dots.length - 1;
		const next =
			event.key === 'ArrowLeft'
				? Math.max(0, stop - 1)
				: event.key === 'ArrowRight'
					? Math.min(last, stop + 1)
					: event.key === 'Home'
						? 0
						: event.key === 'End'
							? last
							: null;
		if (next === null) return;
		event.preventDefault();
		current = next;
		buttons[next]?.focus();
	}
</script>

<!-- The thread's shape in time: one dot a day it holds notes, from its first
     day to today, so how long it ran, how often it came up and how long it
     has been quiet read at a glance. A dot brings its day into sight. -->
<div class="mb-6" style:--hue={threadHue(thread.id)}>
	<!-- Fainter while only suggested, as its lane is among the threads. -->
	<div
		role="toolbar"
		aria-label={m.thread_arc()}
		aria-orientation="horizontal"
		tabindex="-1"
		bind:clientWidth={width}
		onkeydown={onKeydown}
		class={['relative mx-2 h-6 outline-none', !thread.kept && 'opacity-50']}
	>
		<span class="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-neutral-800"></span>
		<span
			class="name-mark absolute top-1/2 h-0.5 -translate-y-1/2 rounded-full opacity-40"
			style:left="{at(thread.since)}%"
			style:right="{100 - at(thread.until)}%"
		></span>
		{#each dots as dot, i (dot.from)}
			<button
				bind:this={buttons[i]}
				type="button"
				tabindex={i === stop ? 0 : -1}
				onclick={() => onday(dot.from)}
				onfocus={() => (current = i)}
				aria-label={label(dot)}
				title={label(dot)}
				class="group absolute top-1/2 flex size-5 -translate-1/2 cursor-pointer items-center justify-center rounded-full outline-none focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring focus-visible:outline-solid"
				style:left="{dot.at}%"
			>
				<span
					class={[
						'name-mark rounded-full ring-2 ring-background transition-transform group-hover:scale-150',
						size(dot.notes)
					]}
				></span>
			</button>
		{/each}
	</div>
	<div class="mx-2 flex justify-between text-xs text-meta tabular-nums" aria-hidden="true">
		<span>{shortDay(start)}</span>
		<span>{shortDay(today)}</span>
	</div>
</div>
