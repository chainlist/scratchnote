<script lang="ts">
	import type { MapNote, Note, Thread } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { shortDay } from '#lib/dates.js';
	import { mentionHue } from '#lib/mentions.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { threadHue } from '#lib/threads.js';
	import type { Point } from './map-data.js';

	let {
		note,
		preview,
		thread,
		at,
		width,
		height,
		nameOf,
		size = $bindable({ width: 0, height: 0 })
	}: {
		/** The note pointed at. */
		note: MapNote;
		/** Its text, once read. */
		preview: Note | null | undefined;
		/** The thread it is in. */
		thread: Thread | undefined;
		/** Where its dot is on the canvas. */
		at: Point;
		/** The map's size, which the card keeps inside. */
		width: number;
		height: number;
		nameOf: (key: string) => string;
		/** The card's own size, which grows with the text size, to keep it
		 *  beside the dot and inside the map. Kept by the map from one card to
		 *  the next, so a new card is placed by the last one's size. */
		size?: { width: number; height: number };
	} = $props();

	const shell = getShell();
</script>

<!-- Below and right of the dot, or on whichever side has the room. -->
<div
	bind:offsetWidth={size.width}
	bind:offsetHeight={size.height}
	class="pointer-events-none absolute z-10 w-72 max-w-[calc(100%-1rem)] rounded-md border bg-popover px-3 py-2 text-sm text-popover-foreground"
	style:left="{Math.max(0, at.x + 14 + size.width > width ? at.x - 14 - size.width : at.x + 14)}px"
	style:top="{Math.max(
		0,
		at.y + 14 + size.height > height ? at.y - 14 - size.height : at.y + 14
	)}px"
>
	<p class="text-xs text-muted-foreground">{shortDay(note.date)}</p>
	{#if preview?.kind === 'page' && preview.subject}
		<p class="font-medium">{preview.subject}</p>
	{/if}
	{#if preview}
		<Markdown text={preview.body} links={false} class="mt-0.5 line-clamp-3" />
	{/if}
	{#if note.mentions.length}
		<p class="mt-1.5 flex flex-wrap gap-1">
			{#each note.mentions as key (key)}
				<span class="name-tint rounded-md px-1 text-xs font-medium" style:--hue={mentionHue(key)}
					>{nameOf(key)}</span
				>
			{/each}
		</p>
	{/if}
	{#if thread}
		<p class="mt-1.5 flex items-center gap-1.5 text-xs text-muted-foreground">
			<span class="name-mark size-2 shrink-0 rounded-full" style:--hue={threadHue(thread.id)}
			></span>
			<span class="truncate">{shell.threads.nameOf(thread)}</span>
		</p>
	{/if}
</div>
