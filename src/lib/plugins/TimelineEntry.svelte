<script lang="ts">
	import type { Attachment } from 'svelte/attachments';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import TimelineItem from '#lib/components/TimelineItem.svelte';
	import { dayPart, minutesBetween } from '#lib/days.js';
	import { call } from './setting.svelte';
	import type { TimelineItemModel } from './timeline.svelte';

	/** One item of a plugin's `Timeline`; the builder changes the model it draws. */
	let {
		model,
		previous,
		contentEl
	}: {
		model: TimelineItemModel;
		/** The item before it on the timeline, if any. */
		previous: () => TimelineItemModel | undefined;
		contentEl: HTMLElement;
	} = $props();

	/** As one day's notes are placed: the part of the day it opens and the
	 *  minutes since the item before. An item with a date spans days and gets
	 *  neither, as its date stands between it and the one before; so does one
	 *  whose time is not a clock's. */
	const spacing = $derived.by(() => {
		const part = model.date ? undefined : dayPart(model.time);
		if (!part) return {};
		const before = previous();
		const usable = before && !before.date && dayPart(before.time) ? before : undefined;
		return {
			part: !usable || dayPart(usable.time) !== part ? part : undefined,
			gapMinutes: usable ? minutesBetween(usable.time, model.time) : 0
		};
	});

	/** The plugin's element, in the body's column. */
	const content: Attachment<HTMLElement> = (el) => {
		el.append(contentEl);
	};
</script>

<TimelineItem
	{...spacing}
	time={model.time}
	date={model.date}
	ontime={model.click && (() => call(model.click, undefined))}
	timeTitle={model.clickTitle}
	icon={model.icon ? mark : undefined}
>
	<div class="min-w-0" {@attach content}></div>
</TimelineItem>

{#snippet mark()}
	<PluginIcon icon={model.icon} class="size-3" />
{/snippet}
