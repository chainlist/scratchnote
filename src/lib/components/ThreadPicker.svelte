<script lang="ts">
	import { threadCards, threadsForNote, type ThreadCard } from '#lib/api.js';
	import * as Command from '#lib/components/ui/command/index.js';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { m } from '#lib/paraglide/messages.js';
	import type { ThreadPick } from '#lib/shell.svelte.js';

	let {
		pick = $bindable(),
		onconfirm
	}: {
		/** What waits on a thread; null closes the picker. */
		pick: ThreadPick | null;
		/** Resolves to an error to show, or null once done. */
		onconfirm: (pick: ThreadPick, into: string | null) => Promise<string | null>;
	} = $props();

	let threads = $state<ThreadCard[]>([]);
	let error = $state<string | null>(null);
	let working = $state(false);
	/** What the picker shows, kept as it was while it fades out. */
	let shown = $state<ThreadPick | null>(null);

	// One note is offered the threads it fits best first; several, or a
	// whole thread, every other thread, the one written in last first.
	$effect(() => {
		const asked = pick;
		if (!asked) return;
		shown = asked;
		threads = [];
		error = null;
		const load =
			asked.kind !== 'merge' && asked.notes.length === 1
				? threadsForNote(asked.notes[0])
				: threadCards();
		load
			.then((found) => {
				if (pick === asked) threads = found.filter((thread) => thread.id !== asked.from);
			})
			.catch((e) => (error = String(e)));
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
	const hint = $derived(
		shown?.kind === 'merge'
			? m.thread_pick_merge_hint()
			: shown?.kind === 'move'
				? m.thread_pick_move_hint()
				: m.thread_pick_add_hint()
	);

	function firstLine(body: string) {
		return body.split('\n').find((line) => line.trim()) ?? '';
	}

	/** A thread's title, else its first note's first line. */
	function name(thread: ThreadCard) {
		const first = thread.first[0];
		return thread.title ?? (first ? (first.subject ?? firstLine(first.body)) : m.thread_untitled());
	}

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
						{m.thread_last({ count: thread.notes.length, date: shortDay(thread.until) })}
						{#if !thread.kept}· {m.threads_suggested()}{/if}
					</span>
				</span>
			</Command.Item>
		{/each}
	</Command.List>
	{#if error}
		<p class="border-t px-4 py-2 text-sm text-destructive">{error}</p>
	{/if}
</Command.Dialog>
