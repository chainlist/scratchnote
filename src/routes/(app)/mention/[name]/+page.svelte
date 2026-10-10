<script lang="ts">
	import MentionLetter from '#lib/components/common/MentionLetter.svelte';
	import NotesByDay from '#lib/components/note/NotesByDay.svelte';
	import SectionTabs from '#lib/components/layout/SectionTabs.svelte';
	import ThreadLanes from '#lib/components/thread/ThreadLanes.svelte';
	import KeepThreadButtons from '#lib/components/thread/KeepThreadButtons.svelte';
	import View from '#lib/components/layout/View.svelte';
	import { shortDay } from '#lib/helpers/dates.js';
	import { mentionHue, mentionKey } from '#lib/notes/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data, params } = $props();

	const shell = getShell();

	// A view being left reads its title once more with the next route's data,
	// as the top bar draws it, which may hold no name: then nothing, no error.
	/** The notes and pages that mention it, newest first. */
	const notes = $derived(data.found?.notes ?? []);
	/** As the newest note types it, or as the link has it once none does. */
	const name = $derived(data.found?.name ?? params.name ?? '');

	/** The threads found among the name's notes (SPEC 6.4), the user's
	 *  first, then those only suggested, each the one written in last first. */
	const threads = $derived(
		shell.threads.threadList
			.filter((thread) => thread.scope === mentionKey(name))
			.toSorted((a, b) => Number(b.kept) - Number(a.kept) || b.until.localeCompare(a.until))
	);

	/** What is shown of the name: its notes, or the threads among them. Kept
	 *  from one name to the next. */
	let showing = $state<'notes' | 'threads'>('notes');
	/** Threads only while they are on offer, with the embedding model. */
	const shown = $derived(shell.canSimilar ? showing : 'notes');

	/** How many notes name it, and the days of the first and the last. */
	const facts = $derived.by(() => {
		if (!notes.length) return null;
		const first = notes.at(-1)!.date;
		const last = notes[0].date;
		return [
			m.tags_notes({ count: notes.length }),
			first === last
				? shortDay(last)
				: m.thread_range({ since: shortDay(first), until: shortDay(last) })
		].join(' · ');
	});
</script>

<!-- A name as a page of its own: its letter in its colour, how many notes
     name it and over which days, then its notes as a calendar reads them,
     or the threads found among them (SPEC 3.10). Keyed by the name, so
     another one rises into place. -->
<View
	back={shell.back}
	title={`@${name}`}
	pin={{ kind: 'mention', target: mentionKey(name), label: name }}
	key={mentionKey(params.name ?? '')}
>
	{#snippet heading(compact: boolean)}
		{#if compact}
			<span class="min-w-0 truncate text-sm font-semibold">@{name}</span>
		{:else}
			<MentionLetter
				{name}
				large
				class="ml-1"
				hue={mentionHue(mentionKey(name))}
				aria-hidden="true"
			/>
			<div class="ml-1.5 min-w-0">
				<h1 class="truncate text-2xl leading-8 font-semibold tracking-tight">@{name}</h1>
				{#if facts}
					<p class="truncate text-sm text-muted-foreground">{facts}</p>
				{/if}
			</div>
		{/if}
	{/snippet}

	{#if shell.canSimilar}
		<!-- Its notes, or the threads among them, each with how many. -->
		<SectionTabs
			bind:value={showing}
			tabs={[
				{ value: 'notes', label: m.mention_notes(), count: notes.length },
				{ value: 'threads', label: m.threads_all(), count: threads.length }
			]}
		/>
	{/if}

	{#if shown === 'threads'}
		{#if threads.length}
			<ThreadLanes {threads} chips={false}>
				{#snippet actions(thread)}
					{#if !thread.kept}
						<!-- A suggestion is kept or dismissed here, as in Threads. -->
						<KeepThreadButtons {thread} keepClass="mr-1" />
					{/if}
				{/snippet}
			</ThreadLanes>
		{:else}
			<p class="text-base text-meta">{m.mention_threads_none()}</p>
		{/if}
	{:else if notes.length}
		<NotesByDay {notes} {...shell.cardActions} />
	{:else}
		<p class="text-base text-meta">{m.mentions_nothing({ name: `@${name}` })}</p>
	{/if}
</View>
