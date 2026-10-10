<script lang="ts">
	import { untrack } from 'svelte';
	import { getNotes, type Note, type Thread, type ThreadCard } from '#lib/api.js';
	import KeepThreadButtons from '#lib/components/KeepThreadButtons.svelte';
	import Markdown from '#lib/components/Markdown.svelte';
	import SectionTabs from '#lib/components/SectionTabs.svelte';
	import ShowMore from '#lib/components/ShowMore.svelte';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/dates.js';
	import { mentionHue } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { leadNotes, SUGGESTED_PAGE, threadHref, threadName } from '#lib/threads.js';
	import ThreadLanes from '#lib/components/ThreadLanes.svelte';

	let { data } = $props();

	const shell = getShell();

	/** First notes read since the page loaded, as more suggestions show. */
	let more = $state<Note[]>([]);
	const notes = $derived(new Map([...more, ...data.notes].map((note) => [note.id, note])));
	const card = (thread: Thread): ThreadCard => ({
		...thread,
		first: thread.notes.slice(0, 2).flatMap((id) => notes.get(id) ?? [])
	});

	/** The section shown: suggestions first, as they wait on a decision
	 *  (SPEC 6.4); with none, the user's threads. */
	let tab = $state<'yours' | 'suggested'>(
		untrack(() => data.threads.some((thread) => !thread.kept)) ? 'suggested' : 'yours'
	);

	const suggested = $derived(data.threads.filter((thread) => !thread.kept));
	const yours = $derived(data.threads.filter((thread) => thread.kept).map(card));

	/** How many suggestions are drawn: a space can hold hundreds. */
	let shown = $state(SUGGESTED_PAGE);
	const drawn = $derived(suggested.slice(0, shown).map(card));

	/** Read the first notes of the first `count` suggestions not read yet.
	 *  Each is asked for once, even while on its way or gone since. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- nothing is drawn from it
	const asked = new Set<string>();
	async function readFirst(count: number) {
		const ids = leadNotes(suggested.slice(0, count)).filter(
			(id) => !notes.has(id) && !asked.has(id)
		);
		if (!ids.length) return;
		ids.forEach((id) => asked.add(id));
		const found = await getNotes(ids);
		more = [...more, ...found];
	}

	// Once the list changes under them, as a suggestion is kept or dismissed,
	// the ones moving up into view need their notes too.
	$effect(() => void readFirst(shown).catch((e) => shell.fail(m.error_load_threads(), e)));

	async function showMore() {
		try {
			await readFirst(shown + SUGGESTED_PAGE);
		} catch (e) {
			shell.fail(m.error_load_threads(), e);
		}
		shown += SUGGESTED_PAGE;
	}

	/** A thread's name, as everywhere: its title, else its first note's first line. */
	const name = (thread: ThreadCard) => threadName(thread, thread.first[0]);
</script>

<!-- The space's threads: the user's, the one written in last first, or those
     only suggested, to keep or dismiss (SPEC 6.4). A suggestion is a card
     with its first notes, which opens it, and Keep and Dismiss on its top
     line. -->
<View back={shell.back} title={m.threads_all()}>
	{#if data.threads.length === 0}
		<p class="text-base text-meta">{m.threads_none()}</p>
	{:else}
		<SectionTabs
			bind:value={tab}
			tabs={[
				{ value: 'yours', label: m.threads_yours(), count: yours.length },
				{ value: 'suggested', label: m.threads_suggested(), count: suggested.length }
			]}
		/>
		{#if tab === 'yours'}
			{#if yours.length}
				<ThreadLanes threads={yours} {name} />
			{:else}
				<p class="text-sm text-meta">{m.threads_yours_none()}</p>
			{/if}
		{:else}
			{#if suggested.length}
				<p class="mb-4 text-sm text-meta">{m.threads_suggested_hint()}</p>
				<ul class="flex flex-col gap-3">
					{#each drawn as thread (thread.id)}
						<li
							class="relative rounded-lg border border-neutral-800 px-4 py-3 transition-colors hover:border-neutral-700 hover:bg-neutral-900 has-focus-visible:border-neutral-700 has-focus-visible:bg-neutral-900"
						>
							<div class="flex items-center gap-2">
								<span class="min-w-0 flex-1 truncate text-xs text-meta">
									{#if thread.mention}<span
											class="name-tint mr-1.5 rounded-md px-1 font-medium"
											style:--hue={thread.scope && mentionHue(thread.scope)}>@{thread.mention}</span
										>{/if}
									{m.thread_detail({ count: thread.notes.length, date: shortDay(thread.since) })}
								</span>
								<div class="relative z-10 flex shrink-0 gap-1">
									<KeepThreadButtons {thread} />
								</div>
							</div>
							<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- threadHref resolves it -->
							<a
								href={threadHref(thread.id)}
								class="mt-1 block text-sm text-neutral-300 after:absolute after:inset-0"
							>
								{#each thread.first as note (note.id)}
									<Markdown text={note.body} links={false} class="mt-1 line-clamp-1" />
								{/each}
							</a>
						</li>
					{/each}
				</ul>
				{#if shown < suggested.length}
					<ShowMore class="mt-4" onclick={showMore} />
				{/if}
			{:else}
				<p class="text-sm text-meta">{m.threads_suggested_none()}</p>
			{/if}
		{/if}
	{/if}
</View>
