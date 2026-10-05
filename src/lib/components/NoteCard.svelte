<script lang="ts">
	import { tick } from 'svelte';
	import type { Note } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import TimelineItem from '#lib/components/TimelineItem.svelte';
	import MarkdownEditor from '#lib/components/MarkdownEditor.svelte';
	import DayAhead, { aheadLabel } from '#lib/components/DayAhead.svelte';
	import Recall from '#lib/components/Recall.svelte';
	import ThreadLine from '#lib/components/ThreadLine.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import CalendarXIcon from '@lucide/svelte/icons/calendar-x';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import FolderInputIcon from '@lucide/svelte/icons/folder-input';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import RouteIcon from '@lucide/svelte/icons/route';
	import RouteOffIcon from '@lucide/svelte/icons/route-off';
	import SendHorizontalIcon from '@lucide/svelte/icons/send-horizontal';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		note,
		onedit,
		ondelete,
		onsave,
		onsimilar,
		onpage,
		onmove,
		onerror,
		showDate = false,
		threadLine = true,
		blink = false
	}: {
		note: Note;
		/** Open the full editor. */
		onedit: (note: Note) => void;
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
	const inThread = $derived(shell.threadOf(note.id) !== undefined);
	const keptOut = $derived(shell.keptOut(note.id));

	async function startEditing() {
		if (editing) return;
		draft = body;
		editing = true;
		await tick();
		editor?.focus();
	}

	async function save() {
		if (saving) return;
		saving = true;
		const saved = await onsave(note, draft);
		saving = false;
		if (saved) editing = false;
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
		'cursor-pointer rounded px-1.5 py-0.5 text-[0.625rem] tracking-wide text-neutral-400 uppercase hover:bg-neutral-800 hover:text-neutral-200 disabled:opacity-50';
	const menuButton =
		'cursor-pointer rounded px-1 py-0.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200';

	function onEditKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			editing = false;
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

<!-- The double click is a mouse shortcut; Edit in the menu opens the same
     text from the keyboard. -->
<TimelineItem
	data-note-id={note.id}
	time={note.time}
	date={showDate ? note.date : undefined}
	class={[menuOpen && 'bg-neutral-900', blink && 'note-blink']}
	ondblclick={onDoubleClick}
	onmousedown={onMouseDown}
	onpointerenter={arm}
	onfocusin={arm}
>
	<div class="relative min-w-0">
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
						class="mr-auto min-w-0 text-xs text-neutral-500"
					>
						<span class="mr-auto text-[0.625rem] text-neutral-600">{m.note_edit_hint()}</span>
					</Recall>
					<button type="button" onclick={() => (editing = false)} class={action}
						>{m.common_cancel()}</button
					>
					<Button
						size="icon-sm"
						onclick={save}
						disabled={saving}
						aria-label={m.common_save()}
						title={m.common_save()}
					>
						<SendHorizontalIcon />
					</Button>
				</div>
			</div>
		{:else}
			<Markdown text={body} onchange={saveTicks} class="text-base leading-7 text-neutral-200" />
			<!-- The day the note looks forward to, and the thread it is in. -->
			{#if note.on || (threadLine && inThread)}
				<div class="mt-1.5 flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1">
					{#if note.on}<DayAhead on={note.on} />{/if}
					{#if threadLine}<ThreadLine id={note.id} />{/if}
				</div>
			{/if}
		{/if}
	</div>

	{#if !editing}
		<div
			class="absolute top-0.5 right-3 flex items-center gap-1.5 bg-neutral-900 pl-2 transition group-hover:opacity-100 focus-within:opacity-100 {menuOpen
				? 'opacity-100'
				: 'opacity-0'}"
		>
			<span class="text-[0.625rem] text-neutral-600 select-none">{m.note_dblclick_hint()}</span>
			{#if !armed}
				<button
					bind:this={standIn}
					type="button"
					aria-label={m.note_actions()}
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
						aria-label={m.note_actions()}
						title={m.note_actions()}
						class={menuButton}
					>
						<EllipsisIcon class="size-3.5" />
					</DropdownMenu.Trigger>
					<DropdownMenu.Content align="end" class="w-52">
						<DropdownMenu.Item onSelect={() => onedit(note)}>
							<PencilIcon />{m.note_edit()}
						</DropdownMenu.Item>
						{#if onsimilar}
							<DropdownMenu.Item onSelect={() => onsimilar(note)}>
								<WaypointsIcon />{m.note_similar()}
							</DropdownMenu.Item>
						{/if}
						<DropdownMenu.Item onSelect={() => onpage(note)}>
							<FileTextIcon />{m.pages_turn_into()}
						</DropdownMenu.Item>
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
