<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { ClassValue, HTMLAttributes } from 'svelte/elements';
	import { clockMinutes, gapRoom, type DayPart } from '#lib/days.js';
	import { m } from '#lib/paraglide/messages.js';

	/**
	 * One item of a timeline, as the day draws its notes: the time hanging in
	 * the margin, the body in the column. A day reads as a page that breathes
	 * with time: the room above an item grows with the time since the one
	 * before it, and the first item of a part of the day names it in the
	 * margin. Every timeline of the app is made of these, the plugins' too
	 * (through `Timeline` in the plugin API), so they all look alike.
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
		/** A small mark before the time, as a page has. */
		icon?: Snippet;
		/** The time brightens, with a tick of the accent, while the pointer or the focus is in it. */
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

	/** The time as software reads it, `09:05`, when it is a clock's. */
	const datetime = $derived.by(() => {
		const minutes = clockMinutes(time);
		if (minutes === null) return undefined;
		const pad = (n: number) => String(n).padStart(2, '0');
		return `${pad(Math.floor(minutes / 60))}:${pad(minutes % 60)}`;
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
		'relative self-start pt-1 text-right text-xs leading-5 text-meta tabular-nums transition-colors @max-[24rem]:pt-0 @max-[24rem]:text-left',
		hover && 'group-focus-within:text-neutral-300 group-hover:text-neutral-300'
	]);
</script>

<!-- The room above stands for the time since the item before; a part of the
     day is named where it begins, in the margin over its first time. -->
<div style:padding-top={room ? `${room}rem` : undefined}>
	{#if part}
		<div
			role="heading"
			aria-level={partLevel}
			class="-mx-3 grid grid-cols-[4.5rem_1fr] gap-x-6 px-3 pt-3 @max-[24rem]:grid-cols-1"
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
			'group relative isolate -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-6 rounded-lg px-3 py-3 outline-none @max-[24rem]:grid-cols-1 @max-[24rem]:gap-y-1',
			className
		]}
	>
		{#if aside}
			<div class="absolute top-3.5 right-[calc(100%+0.25rem)] flex">{@render aside()}</div>
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

<!-- The date for a timeline spanning days, a mark such as a page's, the time,
     and the hover's tick of the accent in the gutter beside it. -->
{#snippet stamp()}
	{#if date}<span class="block">{date}</span>{/if}
	{#if icon}<span aria-hidden="true" class="mr-1 inline-block align-[-0.125em]"
			>{@render icon()}</span
		>{/if}{time}
	{#if hover}
		<span
			aria-hidden="true"
			class="absolute top-[calc(50%+2px)] -right-3 h-3 w-0.5 -translate-y-1/2 rounded-full bg-primary opacity-0 transition-opacity group-focus-within:opacity-100 group-hover:opacity-100 @max-[24rem]:hidden"
		></span>
	{/if}
{/snippet}
