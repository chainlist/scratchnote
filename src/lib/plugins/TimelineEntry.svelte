<script lang="ts">
	import type { Attachment } from 'svelte/attachments';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import TimelineItem from '#lib/components/TimelineItem.svelte';
	import { call } from './setting.svelte';
	import type { TimelineItemModel } from './timeline.svelte';

	/** One item of a plugin's `Timeline`; the builder changes the model it draws. */
	let { model, contentEl }: { model: TimelineItemModel; contentEl: HTMLElement } = $props();

	/** The plugin's element, in the body's column. */
	const content: Attachment<HTMLElement> = (el) => {
		el.append(contentEl);
	};
</script>

<TimelineItem
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
