<script lang="ts">
	import type { Note } from '#lib/api.js';
	import DayAhead from '#lib/components/note/DayAhead.svelte';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import ThreadLine from '#lib/components/thread/ThreadLine.svelte';
	import type { NoteChip } from '#lib/plugins/api.js';
	import { registry } from '#lib/plugins/registry.svelte.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		note,
		threadLine
	}: {
		note: Note;
		/** Name the thread the note is in, as everywhere but in that thread. */
		threadLine: boolean;
	} = $props();

	const shell = getShell();
	const inThread = $derived(shell.threads.threadOf(note.id) !== undefined);
	/** What the plugins add under the text; one failing is left out, not the card. */
	const chips = $derived(
		registry.chips.flatMap(({ plugin, chip }): NoteChip[] => {
			try {
				const made = chip(note);
				return made ? [made] : [];
			} catch (e) {
				console.error(`${plugin}: a note chip failed`, e);
				return [];
			}
		})
	);
</script>

<!-- The day the note looks forward to, the thread it is in, and what plugins add. -->
{#if note.on || (threadLine && inThread) || chips.length > 0}
	<div class="mt-1.5 flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
		{#if note.on}<DayAhead on={note.on} />{/if}
		{#if threadLine}<ThreadLine id={note.id} />{/if}
		{#each chips as chip, at (at)}
			{@const look = 'flex max-w-full min-w-0 items-center gap-1.5 text-xs text-meta'}
			{#if chip.onClick}
				{@const click = chip.onClick}
				<button
					type="button"
					title={chip.title}
					onclick={(event) => {
						event.stopPropagation();
						void click();
					}}
					class="{look} cursor-pointer transition-colors hover:text-neutral-200"
				>
					{#if chip.icon}<PluginIcon icon={chip.icon} class="size-3" />{/if}
					<span class="truncate">{chip.text}</span>
				</button>
			{:else}
				<span title={chip.title} class={look}>
					{#if chip.icon}<PluginIcon icon={chip.icon} class="size-3" />{/if}
					<span class="truncate">{chip.text}</span>
				</span>
			{/if}
		{/each}
	</div>
{/if}
