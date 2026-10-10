<script lang="ts">
	import { threadCards, threadsForNote, type ThreadCard } from '#lib/api.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { shortDay } from '#lib/helpers/dates.js';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import type { ThreadPick } from '#lib/shell/threads.svelte.js';
	import { threadName, threadScope } from '#lib/notes/threads.js';
	import { untrack } from 'svelte';

	let {
		pick = $bindable(),
		onconfirm
	}: {
		/** What waits on a thread; null closes the picker. */
		pick: ThreadPick | null;
		/** Resolves to an error to show, or null once done. */
		onconfirm: (pick: ThreadPick, into: string | null) => Promise<string | null>;
	} = $props();

	const shell = getShell();

	let threads = $state<ThreadCard[]>([]);
	let error = $state<string | null>(null);
	let working = $state(false);
	/** What the picker shows, kept as it was while it fades out. */
	let shown = $state<ThreadPick | null>(null);

	/**
	 * A whole thread's best fits first: each thread scores by its place
	 * among the threads its first, middle and last notes fit best, and the
	 * rest keep the order of the one written in last first. Without the
	 * embedding model nothing scores, so that order is all.
	 */
	async function mergeTargets(notes: string[]): Promise<ThreadCard[]> {
		const sample = [...new Set([notes[0], notes[Math.floor(notes.length / 2)], notes.at(-1)])];
		const [all, ...ranked] = await Promise.all([
			threadCards(),
			...sample.filter((id) => id !== undefined).map(threadsForNote)
		]);
		const score: Record<string, number> = {};
		for (const list of ranked)
			list.forEach((thread, i) => (score[thread.id] = (score[thread.id] ?? 0) + 1 / (10 + i)));
		return all.toSorted((a, b) => (score[b.id] ?? 0) - (score[a.id] ?? 0));
	}

	// One note is offered the threads it fits best first, among the names
	// it mentions; a whole thread, those its notes fit best; several notes,
	// every other thread, the one written in last first. Notes from a thread
	// only go to another thread of the same name: a thread of one name never
	// takes in another's. A thread of the user's is never merged into one
	// only suggested, which a slip of the keyboard could otherwise pick.
	$effect(() => {
		const asked = pick;
		if (!asked) return;
		shown = asked;
		threads = [];
		error = null;
		const load =
			asked.kind === 'merge'
				? mergeTargets(asked.notes)
				: asked.notes.length === 1
					? threadsForNote(asked.notes[0])
					: threadCards();
		// Read once: a thread changing while the picker is open must not reload it.
		const fromKept = untrack(
			() => shell.threads.threadList.find((thread) => thread.id === asked.from)?.kept ?? false
		);
		load
			.then((found) => {
				if (pick !== asked) return;
				const scope = asked.from === undefined ? undefined : threadScope(asked.from);
				threads = found.filter(
					(thread) =>
						thread.id !== asked.from &&
						(scope === undefined || thread.scope === scope) &&
						!(asked.kind === 'merge' && fromKept && !thread.kept)
				);
			})
			.catch((e) => (error = `${m.error_load_threads()} (${String(e)})`));
	});

	/** A new thread takes two notes at least: one alone is no thread. */
	const canStart = $derived(shown?.kind === 'move' && shown.notes.length > 1);

	const title = $derived(
		shown?.kind === 'merge'
			? m.thread_pick_merge()
			: shown?.kind === 'move'
				? m.thread_pick_move()
				: m.thread_pick_add()
	);
	/** The thread a merge takes the notes of, as it is named everywhere. */
	const fromName = $derived.by(() => {
		const from = shell.threads.threadList.find((thread) => thread.id === shown?.from);
		return from ? shell.threads.nameOf(from) : m.thread_untitled();
	});

	const hint = $derived(
		shown?.kind === 'merge'
			? m.thread_pick_merge_hint({ count: shown.notes.length, from: fromName })
			: shown?.kind === 'move'
				? m.thread_pick_move_hint()
				: m.thread_pick_add_hint()
	);

	/** A thread's name, as everywhere: its title, else its first note's first line. */
	const name = (thread: ThreadCard) => threadName(thread, thread.first[0]);

	async function choose(into: string | null) {
		if (!pick || working) return;
		working = true;
		error = await onconfirm(pick, into);
		working = false;
		if (error === null) pick = null;
	}
</script>

<Command.Dialog
	open={pick !== null}
	onOpenChange={(open) => {
		if (!open) pick = null;
	}}
	{title}
	description={hint}
	class="top-[12%] sm:max-w-xl"
>
	<div class="flex flex-col gap-1 border-b px-4 pt-4 pb-3">
		<p class="text-sm font-medium">{title}</p>
		<p class="text-xs text-muted-foreground">{hint}</p>
	</div>
	<Command.Input placeholder={m.thread_pick_search()} />
	<Command.List class="max-h-[min(24rem,55vh)]">
		<Command.Empty>{m.thread_pick_none()}</Command.Empty>
		{#if canStart}
			<Command.Item value="new" onSelect={() => void choose(null)}>
				<PlusIcon />{m.thread_new()}
			</Command.Item>
		{/if}
		{#each threads as thread (thread.id)}
			<Command.Item
				value={thread.id}
				keywords={[name(thread), ...thread.first.map((note) => note.body)]}
				onSelect={() => void choose(thread.id)}
			>
				<RouteIcon class="self-start" />
				<span class="flex min-w-0 flex-1 flex-col gap-0.5">
					<span class="truncate">{name(thread)}</span>
					<span class="truncate text-xs text-muted-foreground">
						{#if thread.mention}@{thread.mention} ·{/if}
						{m.thread_last({ count: thread.notes.length, date: shortDay(thread.until) })}
						{#if !thread.kept}· {m.threads_suggested()}{/if}
					</span>
				</span>
			</Command.Item>
		{/each}
	</Command.List>
	{#if error}
		<p role="alert" class="border-t px-4 py-2 text-sm text-destructive">{error}</p>
	{/if}
</Command.Dialog>
