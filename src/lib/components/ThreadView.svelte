<script lang="ts">
	import { renameThread, type Note, type Thread } from '#lib/api.js';
	import NotesByDay from '#lib/components/NotesByDay.svelte';
	import RenameThreadDialog from '#lib/components/RenameThreadDialog.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import ListChecksIcon from '@lucide/svelte/icons/list-checks';
	import MergeIcon from '@lucide/svelte/icons/merge';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import { mentionHref } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		found
	}: {
		/** The thread and its notes, oldest first, or null for one that is gone.
		 *  Another thread mounts the view again, with nothing chosen. */
		found: { thread: Thread; notes: Note[] } | null;
	} = $props();

	const shell = getShell();
	const thread = $derived(found?.thread ?? null);

	/** Rename, Select notes and Merge into, in a row under the title. */
	const action =
		'-mx-1.5 flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 hover:bg-neutral-900 hover:text-neutral-200';

	/** The thread's notes in the order the settings ask for. They come
	 *  oldest first, so a day's are together either way. */
	const ordered = $derived.by(() => {
		const notes = found?.notes ?? [];
		return shell.threadOrder === 'newest' ? notes.toReversed() : notes;
	});

	/** The thread being renamed, while the dialog is open. */
	let renaming = $state<Thread | null>(null);

	/** The notes chosen to move elsewhere, while they are being chosen. */
	let chosen = $state<string[] | null>(null);

	function choose(id: string, on: boolean) {
		if (!chosen) return;
		chosen = on ? [...chosen, id] : chosen.filter((other) => other !== id);
	}

	function moveChosen() {
		if (!thread || !chosen?.length) return;
		shell.picking = { kind: 'move', notes: chosen, from: thread.id };
		chosen = null;
	}

	/** Resolves to an error for the dialog to show, or null once renamed. */
	async function rename(target: Thread, title: string): Promise<string | null> {
		if (title.trim() === (target.named ? target.title : '')) return null;
		try {
			await renameThread(target.id, title);
			return null;
		} catch (e) {
			return String(e);
		}
	}
</script>

<!-- One thread's notes, oldest first so they read as it went, or newest
     first, each day under its own heading with a rail of its own (SPEC 6.4).
     In the timeline's place or in the dock. -->
{#if thread && found}
	{#if !thread.kept}
		<div
			class="mb-6 flex items-center justify-between gap-4 rounded-lg border border-neutral-800 px-4 py-3 text-sm text-neutral-400"
		>
			<span>{m.thread_suggested_banner()}</span>
			<span class="flex shrink-0 gap-2">
				<Button variant="ghost" size="sm" onclick={() => void shell.keepThread(thread.id, false)}>
					{m.thread_dismiss()}
				</Button>
				<Button variant="outline" size="sm" onclick={() => void shell.keepThread(thread.id, true)}>
					{m.thread_keep()}
				</Button>
			</span>
		</div>
	{/if}
	<div class="mb-6 flex min-h-7 items-center gap-3 text-sm text-neutral-500">
		{#if chosen}
			<span>{m.thread_selected({ count: chosen.length })}</span>
			<Button variant="outline" size="sm" disabled={!chosen.length} onclick={moveChosen}>
				{m.thread_move_to()}
			</Button>
			<Button variant="ghost" size="sm" onclick={() => (chosen = null)}>
				{m.common_cancel()}
			</Button>
		{:else}
			{#if thread.mention}
				<!-- The name it was found among the notes of (SPEC 6.4). -->
				<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
				<a href={mentionHref(thread.mention)} class="mention">@{thread.mention}</a>
			{/if}
			<button type="button" onclick={() => (renaming = thread)} class={action}>
				<PencilIcon class="size-3.5" />{m.thread_rename()}
			</button>
			<button type="button" onclick={() => (chosen = [])} class={action}>
				<ListChecksIcon class="size-3.5" />{m.thread_select()}
			</button>
			<button
				type="button"
				onclick={() => (shell.picking = { kind: 'merge', notes: thread.notes, from: thread.id })}
				class={action}
			>
				<MergeIcon class="size-3.5" />{m.thread_merge()}
			</button>
		{/if}
	</div>
	<NotesByDay
		notes={ordered}
		threadLine={false}
		{chosen}
		onchoose={choose}
		{...shell.cardActions}
	/>
{:else}
	<p class="text-base text-neutral-600">{m.thread_gone()}</p>
{/if}

<RenameThreadDialog bind:thread={renaming} onconfirm={rename} />
