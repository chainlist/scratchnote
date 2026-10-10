<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import type { MapNote, Note, Thread } from '#lib/api.js';
	import MentionChip from '#lib/components/MentionChip.svelte';
	import Markdown from '#lib/components/Markdown.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import { shortDay } from '#lib/dates.js';
	import { mentionHref, mentionHue } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { threadHref, threadHue } from '#lib/threads.js';

	let {
		note,
		preview,
		thread,
		nameOf,
		onclose,
		openButton = $bindable(null)
	}: {
		/** The note clicked on the map. */
		note: MapNote;
		/** Its text, once read. */
		preview: Note | null | undefined;
		/** The thread it is in. */
		thread: Thread | undefined;
		nameOf: (key: string) => string;
		onclose: () => void;
		/** Its Open button, which a note picked from the search's list focuses. */
		openButton?: HTMLElement | null;
	} = $props();

	const shell = getShell();
</script>

<Card.Root
	size="sm"
	class="absolute top-12 right-2 z-10 max-h-[calc(100%-3.5rem)] w-80 @max-[38rem]:top-auto @max-[38rem]:bottom-2 @max-[38rem]:left-2 @max-[38rem]:max-h-[calc(45%-1rem)] @max-[38rem]:w-auto"
>
	<Card.Header>
		<Card.Description class="text-xs">{shortDay(note.date)}</Card.Description>
		{#if preview?.kind === 'page' && preview.subject}
			<Card.Title>{preview.subject}</Card.Title>
		{/if}
		<Card.Action>
			<Button
				variant="ghost"
				size="icon-xs"
				aria-label={m.common_close()}
				title={m.common_close()}
				onclick={onclose}
			>
				<XIcon />
			</Button>
		</Card.Action>
	</Card.Header>
	{#if preview}
		<Card.Content class="min-h-0 overflow-y-auto">
			<Markdown text={preview.body} links={false} />
		</Card.Content>
	{/if}
	{#if note.mentions.length}
		<Card.Content class="flex flex-wrap gap-1">
			{#each note.mentions as key (key)}
				<MentionChip href={mentionHref(key)} class="text-sm hover:underline" hue={mentionHue(key)}
					>{nameOf(key)}</MentionChip
				>
			{/each}
		</Card.Content>
	{/if}
	<Card.Content class="flex items-center gap-2">
		{#if thread}
			<span class="name-mark size-2.5 shrink-0 rounded-full" style:--hue={threadHue(thread.id)}
			></span>
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- threadHref resolves it -->
			<a
				href={threadHref(thread.id)}
				class="min-w-0 flex-1 truncate text-xs text-muted-foreground hover:underline"
			>
				{shell.threads.nameOf(thread)}
			</a>
		{/if}
		<Button
			bind:ref={openButton}
			size="sm"
			class="ml-auto"
			onclick={() => void shell.openCited(note)}
		>
			{m.map_open()}
		</Button>
	</Card.Content>
</Card.Root>
