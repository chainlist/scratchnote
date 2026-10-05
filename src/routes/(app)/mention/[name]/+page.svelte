<script lang="ts">
	import type { Note } from '#lib/api.js';
	import NoteList from '#lib/components/NoteList.svelte';
	import ThreadLanes from '#lib/components/ThreadLanes.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import View from '#lib/components/View.svelte';
	import { dayHeading, shortDay } from '#lib/components/ViewHeader.svelte';
	import { mentionKey } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data, params } = $props();

	const shell = getShell();
	/** As the newest note types it, or as the link has it once none does. */
	const name = $derived(data.found.name ?? params.name);

	/** The threads found among the name's notes (SPEC 6.4), the user's
	 *  first, then those only suggested, each the one written in last first. */
	const threads = $derived(
		shell.threadList
			.filter((thread) => thread.scope === mentionKey(name))
			.toSorted((a, b) => Number(b.kept) - Number(a.kept) || b.until.localeCompare(a.until))
	);

	/** What is shown of the name: its notes, or the threads among them. Kept
	 *  from one name to the next. */
	let showing = $state<'notes' | 'threads'>('notes');
	/** Threads only while they are on offer, with the embedding model. */
	const shown = $derived(shell.canSimilar ? showing : 'notes');

	/** The notes by day, newest first, as they come. */
	const days = $derived.by(() => {
		const days: { date: string; notes: Note[] }[] = [];
		for (const note of data.found.notes) {
			const last = days.at(-1);
			if (last?.date === note.date) last.notes.push(note);
			else days.push({ date: note.date, notes: [note] });
		}
		return days;
	});
</script>

<!-- The notes and pages that mention a name, each day under its own heading,
     or the threads found among them (SPEC 3.10). Keyed by the name, so
     another one rises into place. -->
<View
	back={shell.back}
	title={`@${name}`}
	detail={data.found.notes.length
		? m.thread_detail({
				count: data.found.notes.length,
				date: shortDay(data.found.notes.at(-1)!.date)
			})
		: undefined}
	pin={{ kind: 'mention', target: mentionKey(name), label: name }}
	key={mentionKey(params.name)}
>
	{#if shell.canSimilar}
		<!-- Its notes, or the threads among them, each with how many. -->
		<ToggleGroup.Root
			type="single"
			variant="outline"
			size="sm"
			value={showing}
			onValueChange={(value) => value && (showing = value as 'notes' | 'threads')}
			class="-mt-4 mb-6"
		>
			<ToggleGroup.Item value="notes" class="gap-1.5 px-3">
				{m.mention_notes()}
				<span class="text-xs text-muted-foreground tabular-nums">{data.found.notes.length}</span>
			</ToggleGroup.Item>
			<ToggleGroup.Item value="threads" class="gap-1.5 px-3">
				{m.threads_all()}
				<span class="text-xs text-muted-foreground tabular-nums">{threads.length}</span>
			</ToggleGroup.Item>
		</ToggleGroup.Root>
	{/if}

	{#if shown === 'threads'}
		{#if threads.length}
			<ThreadLanes {threads} chips={false}>
				{#snippet actions(thread)}
					{#if !thread.kept}
						<!-- A suggestion is kept or dismissed here, as in Threads. -->
						<Button
							variant="ghost"
							size="sm"
							class="h-7 px-2 text-xs"
							onclick={() => void shell.keepThread(thread.id, false)}
						>
							{m.thread_dismiss()}
						</Button>
						<Button
							variant="outline"
							size="sm"
							class="mr-1 h-7 px-2 text-xs"
							onclick={() => void shell.keepThread(thread.id, true)}
						>
							{m.thread_keep()}
						</Button>
					{/if}
				{/snippet}
			</ThreadLanes>
		{:else}
			<p class="text-base text-neutral-600">{m.mention_threads_none()}</p>
		{/if}
	{:else}
		{#each days as day (day.date)}
			<section class="mb-8 last:mb-0">
				<h2 class="border-b border-neutral-800 pb-2 text-sm font-medium text-neutral-400">
					{dayHeading(day.date)}
				</h2>
				<NoteList notes={day.notes} empty="" {...shell.cardActions} />
			</section>
		{:else}
			<p class="text-base text-neutral-600">{m.mentions_nothing({ name: `@${name}` })}</p>
		{/each}
	{/if}
</View>
