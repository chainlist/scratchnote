<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue, HTMLAttributes } from 'svelte/elements';
	import { clockMinutes, clockTime, gapRoom, type DayPart } from '#lib/helpers/dates.js';
	import { m } from '#lib/paraglide/messages.js';
	import { pop } from '#lib/helpers/motion.js';

	/**
	 * One item of a timeline, as the day draws its notes: the time hanging in
	 * the margin, the rail with the item's mark on it in the gutter, the body
	 * in the column. A day reads as a page that breathes with time: the room
	 * above an item grows with the time since the one before it, so the rail
	 * runs long through a quiet afternoon, and the first item of a part of the
	 * day names it in the margin. Every timeline of the app is made of these,
	 * the plugins' too (through `Timeline` in the plugin API), so they all
	 * look alike. Each sits in an `li` of the list, which is how the first and
	 * last know to stop the rail at their mark.
	 */
	let {
		time,
		date,
		ontime,
		timeTitle,
		icon,
		hover = true,
		part,
		partLevel = 2,
		gapMinutes = 0,
		aside,
		class: className,
		children,
		...rest
	}: HTMLAttributes<HTMLElement> & {
		time: string;
		/** Above the time, for a timeline that spans days. */
		date?: string;
		/** Makes the time a button, such as to the note on its day. */
		ontime?: () => void;
		timeTitle?: string;
		/** A mark in a box on the rail, as a page has, in place of the dot. */
		icon?: Snippet;
		/** The time brightens and the mark takes the accent while the pointer or the focus is in it. */
		hover?: boolean;
		/** The part of the day this item opens, named above it in the margin. */
		part?: DayPart;
		/** The heading level of that name: one under the view's own headings. */
		partLevel?: number;
		/** Minutes since the item before it, on the same day: room above it to match. */
		gapMinutes?: number;
		/** Left of the margin, level with the first line, such as a box to choose the item. */
		aside?: Snippet;
		class?: ClassValue;
		/** The body's column, and whatever is laid over the item. */
		children: Snippet;
	} = $props();

	/** The room above for the time since the item before; a part's name,
	 *  two rem with its own room, stands in for that much of it. */
	const room = $derived(Math.max(0, gapRoom(gapMinutes) - (part ? 2 : 0)));

	/** Where the mark sits, in rem from the top: below the room and the part's
	 *  name, level with the first line of the time. */
	const markTop = $derived(room + (part ? 2 : 0) + 1.625);

	/** The time as software reads it, `09:05`, when it is a clock's. */
	const datetime = $derived.by(() => {
		const minutes = clockMinutes(time);
		return minutes === null ? undefined : clockTime(minutes);
	});

	const partName: Record<DayPart, () => string> = {
		morning: m.day_part_morning,
		afternoon: m.day_part_afternoon,
		evening: m.day_part_evening,
		night: m.day_part_night
	};

	// In the user's font, with figures of one width so the times still line up.
	// In a narrow container, as a docked thread, the time heads the body instead.
	const when = $derived([
		'self-start pt-1 text-right text-xs leading-5 text-meta tabular-nums transition-colors @max-[24rem]:pt-0 @max-[24rem]:text-left',
		hover && 'group-focus-within:text-neutral-300 group-hover:text-neutral-300'
	]);
</script>

<!-- The room above stands for the time since the item before; a part of the
     day is named where it begins, in the margin over its first time. The
     item measures its own width, so it folds to one column wherever it is
     narrow: a docked view, a plugin's panel, a thread's day. -->
<div class="@container relative" style:padding-top={room ? `${room}rem` : undefined}>
	<!-- The rail in the gutter, through the room and the part's name: each item
	     draws the segments above and below its mark, and the first and last
	     leave off the outer ends so the rail stops at their marks. In a narrow
	     container the time heads the body and there is no gutter for it. -->
	<span
		aria-hidden="true"
		class="tl-rail-above absolute top-0 left-[5.25rem] w-px -translate-x-1/2 bg-neutral-800 @max-[24rem]:hidden [li:first-child_&]:hidden"
		style:height="{markTop}rem"
	></span>
	<span
		aria-hidden="true"
		class="tl-rail-below absolute bottom-0 left-[5.25rem] w-px -translate-x-1/2 bg-neutral-800 @max-[24rem]:hidden [li:last-child_&]:hidden"
		style:top="{markTop}rem"
	></span>
	{#if part}
		<div
			role="heading"
			aria-level={partLevel}
			class="tl-part -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-6 px-3 pt-3 @max-[24rem]:grid-cols-1"
		>
			<span
				class="text-right text-xs leading-5 font-medium whitespace-nowrap text-neutral-400 @max-[24rem]:text-left"
			>
				{partName[part]()}
			</span>
		</div>
	{/if}
	<article
		{...rest}
		class={[
			'tl-item group relative isolate -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-6 rounded-lg px-3 py-3 outline-none @max-[24rem]:grid-cols-1 @max-[24rem]:gap-y-1',
			className
		]}
	>
		{#if aside}
			<div class="absolute top-3.5 right-[calc(100%+0.25rem)] flex" in:pop>{@render aside()}</div>
		{/if}
		<!-- The item's mark on the rail: a dot, or its icon in a box. -->
		{#if icon}
			<span
				aria-hidden="true"
				class={[
					'tl-mark absolute top-[1.625rem] left-24 flex size-4.5 -translate-1/2 items-center justify-center rounded border border-neutral-700 bg-neutral-950 text-meta transition-[color,border-color,scale] duration-200 ease-settle @max-[24rem]:hidden',
					hover &&
						'group-focus-within:scale-110 group-focus-within:border-primary group-focus-within:text-primary group-hover:scale-110 group-hover:border-primary group-hover:text-primary'
				]}
			>
				{@render icon()}
			</span>
		{:else}
			<span
				aria-hidden="true"
				class={[
					'tl-mark absolute top-[1.625rem] left-24 size-2 -translate-1/2 rounded-full border border-neutral-700 bg-neutral-950 transition-[background-color,border-color,scale] duration-200 ease-settle @max-[24rem]:hidden',
					hover &&
						'group-focus-within:scale-150 group-focus-within:border-primary group-focus-within:bg-primary group-hover:scale-150 group-hover:border-primary group-hover:bg-primary'
				]}
			></span>
		{/if}
		{#if ontime}
			<button
				type="button"
				onclick={ontime}
				title={timeTitle}
				class={[when, 'cursor-pointer hover:text-neutral-200']}
			>
				{@render stamp()}
			</button>
		{:else}
			<time {datetime} class={when}>{@render stamp()}</time>
		{/if}
		{@render children()}
	</article>
</div>

<!-- The date for a timeline spanning days, then the time. In a narrow
     container, with no rail, a mark such as a page's leads the time. -->
{#snippet stamp()}
	{#if date}<span class="block">{date}</span>{/if}
	{#if icon}<span aria-hidden="true" class="mr-1 hidden align-[-0.125em] @max-[24rem]:inline-block"
			>{@render icon()}</span
		>{/if}{time}
{/snippet}
