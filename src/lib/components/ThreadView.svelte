<script lang="ts">
	import { tick } from 'svelte';
	import { renameThread, type Note, type Thread, type ThreadOrder } from '#lib/api.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import NotesByDay from '#lib/components/note/NotesByDay.svelte';
	import RenameThreadDialog from '#lib/components/RenameThreadDialog.svelte';
	import ThreadArc from '#lib/components/ThreadArc.svelte';
	import { shortDay } from '#lib/dates.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { openTasks } from '#lib/threads.js';
	import { resolve } from '$app/paths';
	import ArrowDownUpIcon from '@lucide/svelte/icons/arrow-down-up';
	import ArrowRightIcon from '@lucide/svelte/icons/arrow-right';
	import ListChecksIcon from '@lucide/svelte/icons/list-checks';
	import MergeIcon from '@lucide/svelte/icons/merge';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import { mentionHref } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { closeOnBack } from '#lib/back.svelte.js';

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
		'-mx-1.5 flex cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 outline-none hover:bg-neutral-900 hover:text-neutral-200 focus-ring';

	/** The thread's notes in the order the settings ask for. They come
	 *  oldest first, so a day's are together either way. */
	const ordered = $derived.by(() => {
		const notes = found?.notes ?? [];
		return shell.threads.threadOrder === 'newest' ? notes.toReversed() : notes;
	});

	const flipOrder = () =>
		void shell.threads.setThreadOrder(shell.threads.threadOrder === 'newest' ? 'oldest' : 'newest');

	const merge = (target: Thread) =>
		(shell.threads.picking = { kind: 'merge', notes: target.notes, from: target.id });

	const orderLabel = () =>
		shell.threads.threadOrder === 'newest'
			? m.settings_thread_order_newest()
			: m.settings_thread_order_oldest();

	/** The tasks still open in the thread's notes, as they come: where it stands now. */
	const open = $derived(openTasks(found?.notes ?? []));

	/** This view, which a dock and the thread's page may both hold: a note is
	 *  looked for in its own. */
	let root = $state<HTMLElement>();

	/** Bring a day of the thread into sight, its first note blinking. */
	function showDay(date: string) {
		const first = found?.notes.find((note) => note.date === date);
		if (first) void shell.blink(first.id, root, { focus: true });
	}

	/** The thread being renamed, while the dialog is open. */
	let renaming = $state<Thread | null>(null);

	/** The notes chosen to move elsewhere, while they are being chosen. */
	let chosen = $state<string[] | null>(null);

	function choose(id: string, on: boolean) {
		if (!chosen) return;
		chosen = on ? [...chosen, id] : chosen.filter((other) => other !== id);
	}

	/** The picker leaves the choice alone: closed without a pick, the same
	 *  notes are still chosen. Those it moved leave the thread, and below. */
	function moveChosen() {
		if (!thread || !chosen?.length) return;
		shell.threads.picking = { kind: 'move', notes: chosen, from: thread.id };
	}

	// Notes no longer in the thread, as those just moved, are no longer
	// chosen; once none is left, choosing ends.
	$effect(() => {
		if (!chosen) return;
		const here = new Set(found?.notes.map((note) => note.id));
		const still = chosen.filter((id) => here.has(id));
		if (still.length < chosen.length) chosen = still.length ? still : null;
	});

	/** Where choosing began: Select notes in the row, or the menu holding it
	 *  in a narrow container. */
	let selectButton = $state<HTMLButtonElement>();
	let menuButton = $state<HTMLButtonElement | null>(null);

	/** Stop choosing, the focus going back to where it began, as the box or
	 *  the button that had it is gone. */
	async function stopChoosing() {
		chosen = null;
		await tick();
		const back = [selectButton, menuButton].find((el) => el?.checkVisibility());
		back?.focus();
	}

	// Android's Back too.
	closeOnBack(
		() => chosen !== null,
		() => void stopChoosing()
	);

	/** Escape stops choosing, as Cancel does, unless something open inside
	 *  the view, such as an editor or a menu, takes it first. */
	function onKeydown(event: KeyboardEvent) {
		if (chosen && event.key === 'Escape' && !event.defaultPrevented) {
			event.preventDefault();
			void stopChoosing();
		}
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
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div bind:this={root} class="@container" onkeydown={onKeydown}>
	{#if thread && found}
		<!-- A suggestion says what keeping it does, with Keep as the way on.
	     Renaming, choosing and merging wait until the thread is the user's. -->
		{#if !thread.kept}
			<div
				class="mb-6 flex flex-wrap items-center justify-between gap-x-4 gap-y-3 rounded-lg border border-neutral-800 px-4 py-3 text-sm text-neutral-400"
			>
				<!-- The buttons go under the words when the two do not fit, as in the dock. -->
				<span class="min-w-0 flex-1 basis-48">{m.thread_suggested_banner()}</span>
				<span class="ml-auto flex shrink-0 gap-2">
					<Button
						variant="ghost"
						size="sm"
						onclick={() => void shell.threads.keepThread(thread.id, false)}
					>
						{m.thread_dismiss()}
					</Button>
					<Button size="sm" onclick={() => void shell.threads.keepThread(thread.id, true)}>
						{m.thread_keep()}
					</Button>
				</span>
			</div>
		{/if}
		<!-- In a narrow container, as the dock, the thread's actions fold into one
		     menu at the end of its lane rather than a row of their own. -->
		<div class="flex items-start gap-2">
			<div class="min-w-0 flex-1"><ThreadArc {thread} notes={found.notes} onday={showDay} /></div>
			{#if !chosen}<span class="@[32rem]:hidden">{@render menu()}</span>{/if}
		</div>
		<!-- While notes are being chosen, the row stays at the top as the notes
		     scroll under it, so Move to is in reach from the last of them. Its
		     padding is taken back from its margins, so nothing moves as it sticks. -->
		<div
			class={[
				'flex min-h-7 flex-wrap items-center gap-3 text-sm text-meta',
				chosen
					? 'sticky top-0 z-10 -mx-3 -mt-2 mb-4 border-b border-neutral-800 bg-background px-3 py-2'
					: 'mb-6',
				!chosen && !thread.mention && '@max-[32rem]:hidden'
			]}
		>
			{#if chosen}
				<span>{m.thread_selected({ count: chosen.length })}</span>
				<Button variant="outline" size="sm" disabled={!chosen.length} onclick={moveChosen}>
					{m.thread_move_to()}
				</Button>
				<Button variant="ghost" size="sm" onclick={stopChoosing}>
					{m.common_cancel()}
				</Button>
			{:else}
				{#if thread.mention}
					<!-- The name it was found among the notes of (SPEC 6.4). -->
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
					<a href={mentionHref(thread.mention)} class="mention">@{thread.mention}</a>
				{/if}
				<!-- In a row where there is room, as on the thread's page; in one
				     menu where there is not, as in a narrow dock. -->
				{#if thread.kept}
					<span class="hidden items-center gap-3 @[32rem]:flex">
						<button type="button" onclick={() => (renaming = thread)} class={action}>
							<PencilIcon class="size-3.5" />{m.thread_rename()}
						</button>
						<button
							bind:this={selectButton}
							type="button"
							onclick={() => (chosen = [])}
							class={action}
						>
							<ListChecksIcon class="size-3.5" />{m.thread_select()}
						</button>
						<button type="button" onclick={() => merge(thread)} class={action}>
							<MergeIcon class="size-3.5" />{m.thread_merge()}
						</button>
					</span>
				{/if}
				<!-- The order as it is now, which a click turns around: the setting
				     Settings > Threads has, kept quiet beside the thread's own actions. -->
				<button
					type="button"
					onclick={flipOrder}
					aria-label="{m.settings_thread_order()}: {orderLabel()}"
					class="{action} ml-auto hidden @[32rem]:flex"
				>
					<ArrowDownUpIcon class="size-3.5" />{orderLabel()}
				</button>
			{/if}
		</div>
		<!-- Newest first, where it stands now leads; oldest first, it ends the thread. -->
		{#if shell.threads.threadOrder === 'newest'}{@render stillOpen(false)}{/if}
		<!-- Each month under its name, each day beside its notes, as the
		     mentions list them; the parts of a day are headings under the day's. -->
		<NotesByDay
			notes={ordered}
			threadLine={false}
			partLevel={4}
			blinking={shell.blinking}
			{chosen}
			onchoose={choose}
			{...shell.cardActions}
		/>
		{#if shell.threads.threadOrder !== 'newest'}{@render stillOpen(true)}{/if}
	{:else}
		<!-- Never a dead end: the way on is to the threads still there. -->
		<p class="text-base text-meta">{m.thread_gone()}</p>
		<a
			href={resolve('threads/')}
			class="mt-3 inline-flex items-center gap-1.5 rounded text-sm text-neutral-300 focus-ring outline-none hover:text-neutral-100"
		>
			{m.thread_gone_link()}<ArrowRightIcon class="size-3.5" />
		</a>
	{/if}
</div>

<!-- The thread's actions and its note order in one menu, for a narrow container. -->
{#snippet menu()}
	{#if thread}
		<DropdownMenu.Root>
			<DropdownMenu.Trigger
				bind:ref={menuButton}
				aria-label={m.thread_actions()}
				title={m.thread_actions()}
				class={action}
			>
				<EllipsisIcon class="size-4" />
			</DropdownMenu.Trigger>
			<DropdownMenu.Content align="end" class="w-52">
				{#if thread.kept}
					<DropdownMenu.Item onSelect={() => (renaming = thread)}>
						<PencilIcon />{m.thread_rename()}
					</DropdownMenu.Item>
					<DropdownMenu.Item onSelect={() => (chosen = [])}>
						<ListChecksIcon />{m.thread_select()}
					</DropdownMenu.Item>
					<DropdownMenu.Item onSelect={() => merge(thread)}>
						<MergeIcon />{m.thread_merge()}
					</DropdownMenu.Item>
					<DropdownMenu.Separator />
				{/if}
				<DropdownMenu.Label>{m.settings_thread_order()}</DropdownMenu.Label>
				<DropdownMenu.RadioGroup
					value={shell.threads.threadOrder}
					onValueChange={(order) => void shell.threads.setThreadOrder(order as ThreadOrder)}
				>
					<DropdownMenu.RadioItem value="oldest">
						{m.settings_thread_order_oldest()}
					</DropdownMenu.RadioItem>
					<DropdownMenu.RadioItem value="newest">
						{m.settings_thread_order_newest()}
					</DropdownMenu.RadioItem>
				</DropdownMenu.RadioGroup>
			</DropdownMenu.Content>
		</DropdownMenu.Root>
	{/if}
{/snippet}

<!-- The tasks the thread still holds open, each bringing its note into sight. -->
{#snippet stillOpen(atEnd: boolean)}
	{#if open.length}
		<section class={atEnd ? 'mt-12' : 'mb-10'}>
			<h2 class="text-sm font-medium text-neutral-400">{m.thread_still_open()}</h2>
			<ul class="mt-3 flex flex-col gap-1 pl-24 @max-[24rem]:pl-0">
				{#each open as { note, task }, i (i)}
					<li>
						<button
							type="button"
							onclick={() => void shell.blink(note.id, root, { focus: true })}
							class="-mx-1.5 flex w-full min-w-0 cursor-pointer items-center gap-3 rounded px-1.5 py-1 text-left focus-ring outline-none hover:bg-neutral-900"
						>
							<span
								aria-hidden="true"
								class="size-3.5 shrink-0 rounded-[0.25em] border border-neutral-500"
							></span>
							<span class="min-w-0 flex-1 text-neutral-200">{task}</span>
							<span class="shrink-0 text-xs text-meta tabular-nums">{shortDay(note.date)}</span>
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{/if}
{/snippet}

<RenameThreadDialog bind:thread={renaming} onconfirm={rename} />
