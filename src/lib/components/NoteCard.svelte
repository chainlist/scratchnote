<script lang="ts">
	import { tick } from 'svelte';
	import { fade } from 'svelte/transition';
	import type { Note } from '$lib/api';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import { categoryLabel } from '$lib/categories';
	import { m } from '$lib/paraglide/messages';

	let {
		note,
		onedit,
		ondelete,
		onsave,
		onretry,
		oncategory,
		onsimilar,
		showDate = false,
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
		/** Search results span days, so each card says which one. */
		showDate?: boolean;
		/** Blink once to show where a chat citation led. */
		blink?: boolean;
	} = $props();

	let editing = $state(false);
	let saving = $state(false);
	let draft = $state('');
	let textarea = $state<HTMLTextAreaElement | null>(null);
	let menuOpen = $state(false);

	// Re-running a manual note would be skipped anyway, and a pending one is
	// already in the queue.
	let canRetry = $derived(note.status === 'done' || note.status === 'failed');

	let pending = $derived(note.status === 'pending');
	/** How long the glow takes to come and go, text and edge alike. */
	const glowFade = { duration: 500 };

	async function startEditing() {
		if (editing) return;
		draft = note.body;
		editing = true;
		await tick();
		textarea?.focus();
	}

	async function save() {
		if (saving) return;
		saving = true;
		const saved = await onsave(note, draft);
		saving = false;
		if (saved) editing = false;
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
		if (editing || (event.target as Element).closest('button, a, textarea')) return;
		void startEditing();
	}

	// Without this the second click of a double click selects a word first.
	function onMouseDown(event: MouseEvent) {
		if (!editing && event.detail > 1) event.preventDefault();
	}
</script>

<!-- The double click is a mouse shortcut; Edit in the menu opens the same
     text from the keyboard. A pending note sits above its neighbours: its
     glow spills onto them, and their hover background would cover it. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<article
	data-note-id={note.id}
	class="group relative isolate -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-8 rounded-lg px-3 py-5 transition-colors duration-300 ease-out focus-within:bg-neutral-900 hover:bg-neutral-900 {menuOpen
		? 'bg-neutral-900'
		: ''} {blink ? 'note-blink' : ''} {pending ? 'z-10' : ''}"
	ondblclick={onDoubleClick}
	onmousedown={onMouseDown}
>
	<!-- Enrichment shows only while it matters: a glowing rainbow edge while
	     the note waits for the model, a faint red ring when it failed. -->
	{#if pending}
		<span aria-hidden="true" class="note-aurora" transition:fade={glowFade}></span>
	{:else if note.status === 'failed'}
		<span aria-hidden="true" class="note-error-ring" transition:fade={glowFade}></span>
	{/if}
	<!-- Timeline rail in the gutter between time and body. Each note draws
	     its own dot and the segments above and below it; the first and last
	     notes leave off the outer ends so the rail stops at their dots. -->
	<span
		aria-hidden="true"
		class="absolute top-0 left-[6.25rem] h-[30px] w-px bg-neutral-800 [li:first-child_&]:hidden"
	></span>
	<span
		aria-hidden="true"
		class="absolute top-[30px] left-[6.25rem] size-2 -translate-x-[3.5px] rounded-full border border-neutral-700 bg-neutral-950 transition-colors duration-300 group-hover:border-neutral-400 group-hover:bg-neutral-400"
	></span>
	<span
		aria-hidden="true"
		class="absolute top-[38px] bottom-0 left-[6.25rem] w-px bg-neutral-800 [li:last-child_&]:hidden"
	></span>
	<time class="pt-1 text-right font-mono text-xs leading-5 text-neutral-600">
		{#if showDate}<span class="block">{note.date}</span>{/if}{note.time}
	</time>

	<div class="relative min-w-0">
		{#if editing}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="flex flex-col gap-2" onkeydown={onEditKeydown}>
				<textarea
					bind:this={textarea}
					bind:value={draft}
					rows={Math.min(16, Math.max(3, draft.split('\n').length))}
					aria-label={m.note_body_label()}
					spellcheck="false"
					class="-mx-2 w-[calc(100%+1rem)] resize-y rounded border border-neutral-800 bg-neutral-900 px-2 py-1 text-base leading-7 text-neutral-100 focus:border-neutral-600 focus:outline-none"
				></textarea>
				<div class="flex items-center justify-end gap-1">
					<span class="mr-auto text-[0.625rem] text-neutral-600">{m.note_edit_hint()}</span>
					<button type="button" onclick={() => (editing = false)} class={action}
						>{m.common_cancel()}</button
					>
					<button type="button" onclick={save} disabled={saving} class={action}>
						{saving ? m.common_saving() : m.common_save()}
					</button>
				</div>
			</div>
		{:else}
			<!-- While pending, the text turns transparent and a glowing copy fades
			     in over it, so the two cross-fade both ways. The copy lets clicks
			     and selection through to the real text. data-text feeds the glow
			     drawn behind its letters. -->
			<p
				class="text-base leading-7 whitespace-pre-wrap transition-colors duration-500 {pending
					? 'text-transparent'
					: 'text-neutral-200'}"
			>
				{note.body}
			</p>
			{#if pending}
				<p
					aria-hidden="true"
					data-text={note.body}
					transition:fade={glowFade}
					class="note-glow-text pointer-events-none absolute inset-0 text-base leading-7 whitespace-pre-wrap"
				>
					{note.body}
				</p>
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
			<DropdownMenu.Root bind:open={menuOpen}>
				<DropdownMenu.Trigger
					aria-label={m.note_actions()}
					title={m.note_actions()}
					class="cursor-pointer rounded px-1 py-0.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200"
				>
					<EllipsisIcon class="size-3.5" />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" class="w-40">
					<DropdownMenu.Item onSelect={() => onedit(note)}>
						<PencilIcon />{m.note_edit()}
					</DropdownMenu.Item>
					{#if onsimilar}
						<DropdownMenu.Item onSelect={() => onsimilar(note)}>
							<WaypointsIcon />{m.note_similar()}
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
</article>
