<script lang="ts">
	import { tick } from 'svelte';
	import { fade } from 'svelte/transition';
	import type { Note } from '$lib/api';
	import Markdown from '$lib/components/Markdown.svelte';
	import TimelineItem from '$lib/components/TimelineItem.svelte';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import Recall from '$lib/components/Recall.svelte';
	import ThreadLine from '$lib/components/ThreadLine.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import FolderInputIcon from '@lucide/svelte/icons/folder-input';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import RouteIcon from '@lucide/svelte/icons/route';
	import RouteOffIcon from '@lucide/svelte/icons/route-off';
	import SendHorizontalIcon from '@lucide/svelte/icons/send-horizontal';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import { categoryLabel } from '$lib/categories';
	import { m } from '$lib/paraglide/messages';
	import { getShell } from '$lib/shell.svelte';

	let {
		note,
		onedit,
		ondelete,
		onsave,
		onretry,
		oncategory,
		onsimilar,
		onpage,
		onmove,
		onerror,
		showDate = false,
		threadLine = true,
		blink = false
	}: {
		note: Note;
		/** Open the full editor: body, subject and category. */
		onedit: (note: Note) => void;
		/** Ask to delete; the page confirms before anything is removed. */
		ondelete: (note: Note) => void;
		/** Save a new body. Resolves true once saved; false keeps the editor open. */
		onsave: (note: Note, body: string) => Promise<boolean>;
		onretry: (note: Note) => void;
		/** Filter on a category, as a click on it in the card does. */
		oncategory: (category: string) => void;
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
		/** Blink once to show where a chat citation led. */
		blink?: boolean;
	} = $props();

	let editing = $state(false);
	let saving = $state(false);
	let draft = $state('');
	let editor = $state<MarkdownEditor | null>(null);
	let menuOpen = $state(false);
	/** The body with task boxes ticked but not saved yet, shown meanwhile.
	 *  A plugin's widget can change the text too; it saves the same way. */
	let ticked = $state<string | null>(null);
	let ticking = Promise.resolve();
	const body = $derived(ticked ?? note.body);

	// Re-running a manual note would be skipped anyway, and a pending one is
	// already in the queue.
	let canRetry = $derived(note.status === 'done' || note.status === 'failed');

	const shell = getShell();
	// A pending note glows only while the model is there to label it. With
	// the model off, missing or still downloading, it waits like any other.
	let glowing = $derived(note.status === 'pending' && shell.modelAvailable);
	const inThread = $derived(shell.threadOf(note.id) !== undefined);
	const keptOut = $derived(shell.keptOut(note.id));
	/** How long the glow takes to come and go, text and edge alike. */
	const glowFade = { duration: 500 };

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
</script>

<!-- The double click is a mouse shortcut; Edit in the menu opens the same
     text from the keyboard. A glowing note sits above its neighbours: its
     glow spills onto them, and their hover background would cover it. -->
<TimelineItem
	data-note-id={note.id}
	time={note.time}
	date={showDate ? note.date : undefined}
	class={[menuOpen && 'bg-neutral-900', blink && 'note-blink', glowing && 'z-10']}
	ondblclick={onDoubleClick}
	onmousedown={onMouseDown}
>
	<!-- Enrichment shows only while it matters: a glowing rainbow edge while
	     the note waits for the model, a faint red ring when it failed. -->
	{#if glowing}
		<span aria-hidden="true" class="note-aurora" transition:fade={glowFade}></span>
	{:else if note.status === 'failed'}
		<span aria-hidden="true" class="note-error-ring" transition:fade={glowFade}></span>
	{/if}

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
			<!-- While glowing, the text turns transparent and a glowing copy fades
			     in over it, so the two cross-fade both ways. The copy lets clicks
			     and selection through to the real text, and holds a blurred copy
			     of its own as the glow behind its letters. -->
			<Markdown
				text={body}
				onchange={saveTicks}
				class="text-base leading-7 transition-colors duration-500 {glowing
					? 'text-transparent'
					: 'text-neutral-200'}"
			/>
			{#if glowing}
				<div
					aria-hidden="true"
					transition:fade={glowFade}
					class="note-glow-text pointer-events-none absolute inset-0 text-base leading-7"
				>
					<Markdown text={body} />
					<Markdown text={body} class="note-glow-blur" />
				</div>
			{/if}
			{#if threadLine}<ThreadLine id={note.id} class="mt-1.5" />{/if}
		{/if}
	</div>

	{#if !editing}
		<div
			class="absolute top-0.5 right-3 flex items-center gap-1.5 bg-neutral-900 pl-2 transition group-hover:opacity-100 focus-within:opacity-100 {menuOpen
				? 'opacity-100'
				: 'opacity-0'}"
		>
			<span class="text-[0.625rem] text-neutral-600 select-none">{m.note_dblclick_hint()}</span>
			<DropdownMenu.Root bind:open={menuOpen}>
				<DropdownMenu.Trigger
					aria-label={m.note_actions()}
					title={m.note_actions()}
					class="cursor-pointer rounded px-1 py-0.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200"
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
					{#if inThread}
						<DropdownMenu.Item onSelect={() => void shell.keepOut(note, true)}>
							<RouteOffIcon />{m.thread_leave()}
						</DropdownMenu.Item>
					{:else if keptOut}
						<DropdownMenu.Item onSelect={() => void shell.keepOut(note, false)}>
							<RouteIcon />{m.thread_rejoin()}
						</DropdownMenu.Item>
					{/if}
					{#if canRetry}
						<DropdownMenu.Item onSelect={() => onretry(note)}>
							<RefreshCwIcon />{m.note_rerun()}
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Separator />
					<DropdownMenu.Item variant="destructive" onSelect={() => ondelete(note)}>
						<Trash2Icon />{m.common_delete()}
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</div>

		{#if note.category}
			{@const category = note.category}
			<button
				type="button"
				onclick={() => oncategory(category)}
				title={m.note_show_tag({ tag: categoryLabel(category) })}
				class="absolute right-3 bottom-0.5 cursor-pointer bg-neutral-900 pl-2 font-mono text-[0.625rem] text-neutral-500 opacity-0 transition group-focus-within:opacity-100 group-hover:opacity-100 hover:text-neutral-200"
				>#{categoryLabel(category)}</button
			>
		{/if}
	{/if}
</TimelineItem>
