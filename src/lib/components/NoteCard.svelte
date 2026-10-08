<script lang="ts">
	import { tick, untrack, type ComponentProps } from 'svelte';
	import type { Note } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import TimelineItem from '#lib/components/TimelineItem.svelte';
	import MarkdownEditor from '#lib/components/MarkdownEditor.svelte';
	import InlineError from '#lib/components/InlineError.svelte';
	import DayAhead, { aheadLabel } from '#lib/components/DayAhead.svelte';
	import Recall from '#lib/components/Recall.svelte';
	import ThreadLine from '#lib/components/ThreadLine.svelte';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import type { NoteChip } from '#lib/plugins/api.js';
	import { registry } from '#lib/plugins/registry.svelte.js';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import CalendarXIcon from '@lucide/svelte/icons/calendar-x';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import FolderInputIcon from '@lucide/svelte/icons/folder-input';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import RouteIcon from '@lucide/svelte/icons/route';
	import RouteOffIcon from '@lucide/svelte/icons/route-off';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import { withMention } from '#lib/mentions.js';
	import { editDrafts } from '#lib/note-draft.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		note,
		ondelete,
		onsave,
		onsimilar,
		onpage,
		onmove,
		onerror,
		showDate = false,
		threadLine = true,
		timeline,
		blink = false
	}: {
		note: Note;
		/** Ask to delete; the page confirms before anything is removed. */
		ondelete: (note: Note) => void;
		/** Save a new body. Resolves true once saved; false keeps the editor open. */
		onsave: (note: Note, body: string) => Promise<boolean>;
		/** List the notes closest in meaning. Left out without the embedding model. */
		onsimilar?: (note: Note) => void;
		/** Turn the note into a page; the page asks for its title. */
		onpage: (note: Note) => void;
		/** Move the note to another space; the page asks which. Left out with one space. */
		onmove?: (note: Note) => void;
		/** A file could not be attached while editing. */
		onerror: (message: string) => void;
		/** Search results span days, so each card says which one. */
		showDate?: boolean;
		/** Name the thread the note is in, as everywhere but in that thread. */
		threadLine?: boolean;
		/** Blink once to show where a link to it led. */
		blink?: boolean;
		/** How the timeline places it: the part of the day it opens, the room
		 *  above it, and a box beside it while notes are being chosen. */
		timeline?: Pick<
			ComponentProps<typeof TimelineItem>,
			'part' | 'partLevel' | 'gapMinutes' | 'aside'
		>;
	} = $props();

	let editing = $state(false);
	let saving = $state(false);
	let draft = $state('');
	let editor = $state<MarkdownEditor | null>(null);
	let menuOpen = $state(false);
	/** The menu is drawn once the card is pointed at or focused, as its
	 *  button only shows then: a menu on every card is most of what a long
	 *  list takes to draw. Until then, a plain button stands in for its trigger. */
	let armed = $state(false);
	let standIn = $state<HTMLButtonElement | null>(null);
	let trigger = $state<HTMLElement | null>(null);
	/** The body with task boxes ticked but not saved yet, shown meanwhile.
	 *  A plugin's widget can change the text too; it saves the same way. */
	let ticked = $state<string | null>(null);
	let ticking = Promise.resolve();
	const body = $derived(ticked ?? note.body);

	const shell = getShell();
	/** The note's time, with its day where the list spans days: how a screen reader names it. */
	const when = $derived(showDate ? `${shortDay(note.date)} ${note.time}` : note.time);
	const inThread = $derived(shell.threadOf(note.id) !== undefined);
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
	/** Changes left unsaved when the editor closed, which it opens on again. */
	let kept = $state(untrack(() => editDrafts.get(note.id)));
	/** The last save's error, shown under the note or its editor until a save works. */
	const failure = $derived(shell.saveFailures[note.id]);

	/** Keep these unsaved changes for later, or none. */
	function keep(text?: string) {
		kept = text;
		if (text === undefined) editDrafts.delete(note.id);
		else editDrafts.set(note.id, text);
	}
	const keptOut = $derived(shell.keptOut(note.id));

	async function startEditing() {
		if (editing) return;
		draft = kept ?? body;
		editing = true;
		await tick();
		editor?.focus();
	}

	/** Close the editor; from the keyboard, the focus goes back to the card's menu. */
	async function stopEditing(fromKeyboard: boolean) {
		editing = false;
		if (!fromKeyboard) return;
		await tick();
		(trigger ?? standIn)?.focus();
	}

	/** Esc or Close: the changes are kept, as a new note's draft is, and the
	 *  note offers to go on with them. */
	function close(fromKeyboard: boolean) {
		keep(draft.trim() && draft !== body ? draft : undefined);
		void stopEditing(fromKeyboard);
	}

	async function save(fromKeyboard = false) {
		if (saving) return;
		if (draft.trim() === '') {
			onerror(m.editor_empty());
			return;
		}
		saving = true;
		const saved = await onsave(note, draft);
		saving = false;
		if (!saved) return;
		keep();
		await stopEditing(fromKeyboard);
	}

	/** Edit from the menu waits for it to close, or it takes the focus back. */
	let editAfterMenu = false;

	/** E on a card edits it, as the double click does, from the keyboard. */
	function onCardKeydown(event: KeyboardEvent) {
		if (editing || event.key.toLowerCase() !== 'e') return;
		if (event.ctrlKey || event.metaKey || event.altKey) return;
		const target = event.target as HTMLElement;
		if (target.isContentEditable || target.closest('input, textarea')) return;
		event.preventDefault();
		void startEditing();
	}

	/** A task box clicked saves at once; clicks in a row save one after another. */
	function saveTicks(next: string) {
		ticked = next;
		ticking = ticking.then(async () => {
			const saved = await onsave(note, next);
			if (!saved || ticked === next) ticked = null;
		});
	}

	const action =
		'cursor-pointer rounded px-1.5 py-0.5 text-xs text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200 disabled:opacity-50';
	const menuButton =
		'flex size-6 cursor-pointer items-center justify-center rounded text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200';

	function onEditKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save(true);
		} else if (event.key === 'Escape') {
			event.preventDefault();
			close(true);
		}
	}

	/** A double click anywhere on the card, bar its buttons, edits the body. */
	function onDoubleClick(event: MouseEvent) {
		if (editing || (event.target as Element).closest('button, a')) return;
		void startEditing();
	}

	// Without this the second click of a double click selects a word first.
	function onMouseDown(event: MouseEvent) {
		if (!editing && event.detail > 1) event.preventDefault();
	}

	/** Draw the menu; focus on the stand-in moves to the trigger replacing it. */
	async function arm() {
		if (armed) return;
		const focused = standIn !== null && document.activeElement === standIn;
		armed = true;
		if (focused) {
			await tick();
			trigger?.focus();
		}
	}
</script>

<!-- One way to edit, reached three ways: a double click, Edit in the menu,
     or E while the focus is on the card. -->
<TimelineItem
	{...timeline}
	data-note-id={note.id}
	aria-keyshortcuts="E"
	aria-label={m.note_at({ time: when })}
	time={note.time}
	date={showDate ? note.date : undefined}
	class={[blink && 'note-blink']}
	ondblclick={onDoubleClick}
	onkeydown={onCardKeydown}
	onmousedown={onMouseDown}
	onpointerenter={arm}
	onfocusin={arm}
>
	<!-- A line of about 70 characters at most, as a page of prose has. -->
	<div class="relative max-w-[70ch] min-w-0">
		{#if editing}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="flex flex-col gap-2" onkeydown={onEditKeydown}>
				<MarkdownEditor
					bind:this={editor}
					bind:value={draft}
					label={m.note_body_label()}
					{onerror}
					class="-mx-2 max-h-[calc(16lh+0.5rem)] min-h-[calc(3lh+0.5rem)] w-[calc(100%+1rem)] rounded border border-neutral-800 bg-neutral-900 px-2 py-1 text-base leading-7 text-neutral-100 focus-within:border-neutral-600"
				/>
				<div class="flex items-center justify-end gap-1">
					<Recall
						text={draft}
						initial={note.body}
						exclude={note.id}
						onmention={(name) => (draft = withMention(draft, name))}
						class="mr-auto min-w-0 text-xs text-meta"
					>
						<span class="mr-auto text-xs text-meta">{m.note_new_hint()}</span>
					</Recall>
					<button type="button" onclick={() => close(false)} class={action}
						>{m.common_close()}</button
					>
					<Button size="sm" onclick={() => save()} disabled={saving}>
						{m.common_save()}
					</Button>
				</div>
				{#if failure !== undefined}
					<InlineError message={m.error_save_note()} detail={failure} />
				{/if}
			</div>
		{:else}
			<Markdown text={body} onchange={saveTicks} class="text-base leading-7 text-neutral-200" />
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
			<!-- Changes closed with Esc wait here, never lost to a key. -->
			{#if kept !== undefined}
				<div class="mt-1.5 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs">
					<span class="text-meta">{m.note_edit_kept()}</span>
					<button
						type="button"
						onclick={startEditing}
						class="flex cursor-pointer items-center gap-1.5 text-neutral-300 transition-colors hover:text-neutral-100"
					>
						<PencilIcon class="size-3" />{m.note_edit_continue()}
					</button>
					<button
						type="button"
						onclick={() => keep()}
						class="cursor-pointer text-meta transition-colors hover:text-neutral-200"
					>
						{m.note_draft_discard()}
					</button>
				</div>
			{/if}
			{#if failure !== undefined}
				<InlineError class="mt-1.5" message={m.error_save_note()} detail={failure} />
			{/if}
		{/if}
	</div>

	{#if !editing}
		<!-- For the keyboard only: the shortcut while the note has the focus. -->
		<span
			aria-hidden="true"
			class="absolute top-3.5 right-3 hidden bg-background pl-2 text-xs text-meta select-none group-has-[:focus-visible]:inline"
			>{m.note_key_hint()}</span
		>
		<!-- The note's menu in the margin, beside the time it acts on; over the
		     text's right end when the time heads the text, in a narrow container. -->
		<div
			class="absolute top-3.5 left-1.5 flex transition group-hover:opacity-100 focus-within:opacity-100 @max-[24rem]:top-2.5 @max-[24rem]:right-3 @max-[24rem]:left-auto {menuOpen
				? 'opacity-100'
				: 'opacity-0'}"
		>
			{#if !armed}
				<button
					bind:this={standIn}
					type="button"
					aria-label={m.note_actions_at({ time: when })}
					title={m.note_actions()}
					aria-haspopup="menu"
					aria-expanded="false"
					onclick={() => {
						menuOpen = true;
						void arm();
					}}
					class={menuButton}
				>
					<EllipsisIcon class="size-3.5" />
				</button>
			{:else}
				<DropdownMenu.Root bind:open={menuOpen}>
					<DropdownMenu.Trigger
						bind:ref={trigger}
						aria-label={m.note_actions_at({ time: when })}
						title={m.note_actions()}
						class={menuButton}
					>
						<EllipsisIcon class="size-3.5" />
					</DropdownMenu.Trigger>
					<DropdownMenu.Content
						align="start"
						collisionPadding={8}
						class="w-max max-w-80 min-w-52"
						onCloseAutoFocus={(event) => {
							if (!editAfterMenu) return;
							editAfterMenu = false;
							event.preventDefault();
							void startEditing();
						}}
					>
						<DropdownMenu.Item onSelect={() => (editAfterMenu = true)}>
							<PencilIcon />{m.note_edit()}
							<DropdownMenu.Shortcut>E</DropdownMenu.Shortcut>
						</DropdownMenu.Item>
						{#if onsimilar}
							<DropdownMenu.Item onSelect={() => onsimilar(note)}>
								<WaypointsIcon />{m.note_similar()}
							</DropdownMenu.Item>
						{/if}
						<DropdownMenu.Item onSelect={() => onpage(note)}>
							<FileTextIcon />{m.pages_turn_into()}
						</DropdownMenu.Item>
						<!-- Grouped by what each touches: the note, its thread, where it
						     lives and the day it looks to, then deleting it. -->
						{#if shell.canSimilar || inThread || keptOut}
							<DropdownMenu.Separator />
							{#if shell.canSimilar}
								<DropdownMenu.Item onSelect={() => shell.askThread(note)}>
									<RouteIcon />{inThread ? m.thread_move() : m.thread_add()}
								</DropdownMenu.Item>
							{/if}
							{#if inThread}
								<DropdownMenu.Item onSelect={() => void shell.keepOut(note, true)}>
									<RouteOffIcon />{m.thread_leave()}
								</DropdownMenu.Item>
							{:else if keptOut}
								<DropdownMenu.Item onSelect={() => void shell.keepOut(note, false)}>
									<RouteIcon />{m.thread_rejoin()}
								</DropdownMenu.Item>
							{/if}
						{/if}
						{#if onmove || note.on}
							<DropdownMenu.Separator />
							{#if onmove}
								<DropdownMenu.Item onSelect={() => onmove(note)}>
									<FolderInputIcon />{m.move_to()}
								</DropdownMenu.Item>
							{/if}
							{#if note.on}
								{@const on = note.on}
								<DropdownMenu.Item onSelect={() => void shell.clearDayAhead(note)}>
									<CalendarXIcon />{m.day_ahead_clear({ date: aheadLabel(on) })}
								</DropdownMenu.Item>
							{/if}
						{/if}
						<DropdownMenu.Separator />
						<DropdownMenu.Item variant="destructive" onSelect={() => ondelete(note)}>
							<Trash2Icon />{m.common_delete()}
						</DropdownMenu.Item>
					</DropdownMenu.Content>
				</DropdownMenu.Root>
			{/if}
		</div>
	{/if}
</TimelineItem>
