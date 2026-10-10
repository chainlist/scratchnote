<script lang="ts">
	import { tick, untrack, type ComponentProps } from 'svelte';
	import type { Note } from '#lib/api.js';
	import Markdown from '#lib/components/editor/Markdown.svelte';
	import TimelineItem from '#lib/components/note/TimelineItem.svelte';
	import InlineError from '#lib/components/InlineError.svelte';
	import NoteChips from '#lib/components/note/NoteChips.svelte';
	import NoteEditor from '#lib/components/note/NoteEditor.svelte';
	import NoteMenu from '#lib/components/note/NoteMenu.svelte';
	import { shortDay } from '#lib/dates.js';
	import { typesText } from '#lib/dom.js';
	import PencilIcon from '@lucide/svelte/icons/pencil';
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
	let editor = $state<NoteEditor | null>(null);
	let menu = $state<NoteMenu | null>(null);
	/** The menu is drawn, kept while the editor is open in its place. */
	let armed = $state(false);
	/** The body with task boxes ticked but not saved yet, shown meanwhile.
	 *  A plugin's widget can change the text too; it saves the same way. */
	let ticked = $state<string | null>(null);
	let ticking = Promise.resolve();
	const body = $derived(ticked ?? note.body);

	const shell = getShell();
	/** The note's time, with its day where the list spans days: how a screen reader names it. */
	const when = $derived(showDate ? `${shortDay(note.date)} ${note.time}` : note.time);
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
		menu?.focus();
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

	/** E on a card edits it, as the double click does, from the keyboard. */
	function onCardKeydown(event: KeyboardEvent) {
		if (editing || event.key.toLowerCase() !== 'e') return;
		if (event.ctrlKey || event.metaKey || event.altKey) return;
		if (typesText(event.target as HTMLElement)) return;
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

	/** A double click anywhere on the card, bar its buttons, edits the body. */
	function onDoubleClick(event: MouseEvent) {
		if (editing || (event.target as Element).closest('button, a')) return;
		void startEditing();
	}

	// Without this the second click of a double click selects a word first.
	function onMouseDown(event: MouseEvent) {
		if (!editing && event.detail > 1) event.preventDefault();
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
	onpointerenter={() => menu?.arm()}
	onfocusin={() => menu?.arm()}
>
	<!-- A line of about 70 characters at most, as a page of prose has. -->
	<div class="relative max-w-[70ch] min-w-0">
		{#if editing}
			<NoteEditor
				bind:this={editor}
				bind:value={draft}
				{saving}
				{failure}
				initial={note.body}
				exclude={note.id}
				onsave={(fromKeyboard) => void save(fromKeyboard)}
				onclose={close}
				{onerror}
			/>
		{:else}
			<Markdown text={body} onchange={saveTicks} class="text-base leading-7 text-neutral-200" />
			<NoteChips {note} {threadLine} />
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
		<NoteMenu
			bind:this={menu}
			bind:armed
			{note}
			{when}
			onedit={startEditing}
			{onsimilar}
			{onpage}
			{onmove}
			{ondelete}
		/>
	{/if}
</TimelineItem>
