<script lang="ts">
	import { tick } from 'svelte';
	import type { Note } from '$lib/api';
	import * as DropdownMenu from '$lib/components/ui/dropdown-menu';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import PencilIcon from '@lucide/svelte/icons/pencil';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';

	let {
		note,
		onedit,
		ondelete,
		onsave,
		onretry,
		ontag,
		showDate = false,
		blink = false
	}: {
		note: Note;
		/** Open the full editor: body, subject, category and tags. */
		onedit: (note: Note) => void;
		/** Ask to delete; the page confirms before anything is removed. */
		ondelete: (note: Note) => void;
		/** Save a new body. Resolves true once saved; false keeps the editor open. */
		onsave: (note: Note, body: string) => Promise<boolean>;
		onretry: (note: Note) => void;
		/** Filter on a tag, as a click on it in the card does. */
		ontag: (tag: string) => void;
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
     text from the keyboard. -->
<!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
<article
	data-note-id={note.id}
	class="group relative -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-8 rounded-lg px-3 py-4 transition-colors duration-300 ease-out focus-within:bg-neutral-900 hover:bg-neutral-900 {menuOpen
		? 'bg-neutral-900'
		: ''} {blink ? 'note-blink' : ''}"
	ondblclick={onDoubleClick}
	onmousedown={onMouseDown}
>
	<!-- Timeline rail in the gutter between time and body. Each note draws
	     its own dot and the segments above and below it; the first and last
	     notes leave off the outer ends so the rail stops at their dots. -->
	<span
		aria-hidden="true"
		class="absolute top-0 left-[6.25rem] h-[26px] w-px bg-neutral-800 [li:first-child_&]:hidden"
	></span>
	<span
		aria-hidden="true"
		class="absolute top-[26px] left-[6.25rem] size-2 -translate-x-[3.5px] rounded-full border border-neutral-700 bg-neutral-950 transition-colors duration-300 group-hover:border-neutral-400 group-hover:bg-neutral-400"
	></span>
	<span
		aria-hidden="true"
		class="absolute top-[34px] bottom-0 left-[6.25rem] w-px bg-neutral-800 [li:last-child_&]:hidden"
	></span>
	<time class="pt-1 text-right font-mono text-xs leading-5 text-neutral-600">
		{#if showDate}<span class="block">{note.date}</span>{/if}{note.time}
	</time>

	<div class="min-w-0">
		{#if editing}
			<!-- svelte-ignore a11y_no_static_element_interactions -->
			<div class="flex flex-col gap-2" onkeydown={onEditKeydown}>
				<textarea
					bind:this={textarea}
					bind:value={draft}
					rows={Math.min(16, Math.max(3, draft.split('\n').length))}
					aria-label="Body"
					spellcheck="false"
					class="-mx-2 w-[calc(100%+1rem)] resize-y rounded border border-neutral-800 bg-neutral-900 px-2 py-1 text-[0.9375rem] leading-7 text-neutral-100 focus:border-neutral-600 focus:outline-none"
				></textarea>
				<div class="flex items-center justify-end gap-1">
					<span class="mr-auto text-[0.625rem] text-neutral-600"
						>Ctrl+Enter to save, Esc to cancel</span
					>
					<button type="button" onclick={() => (editing = false)} class={action}>Cancel</button>
					<button type="button" onclick={save} disabled={saving} class={action}>
						{saving ? 'Saving' : 'Save'}
					</button>
				</div>
			</div>
		{:else}
			<p class="text-[0.9375rem] leading-7 whitespace-pre-wrap text-neutral-200">{note.body}</p>
		{/if}
	</div>

	{#if !editing}
		<div
			class="absolute top-3 right-3 flex items-center gap-1.5 bg-neutral-900 pl-2 transition group-hover:opacity-100 focus-within:opacity-100 {menuOpen
				? 'opacity-100'
				: 'opacity-0'}"
		>
			<span class="text-[0.625rem] text-neutral-600 select-none">Double-click to edit</span>
			<!-- A done note says nothing; the others say where enrichment is,
			     and `manual` that the model will leave the note alone. -->
			{#if note.status !== 'done'}
				<span
					title={note.status === 'manual' ? 'Edited by hand, the model leaves it alone' : undefined}
					class="rounded px-1.5 py-0.5 text-[0.625rem] tracking-wide uppercase select-none
						{note.status === 'failed'
						? 'bg-red-50 text-red-600 dark:bg-red-950 dark:text-red-400'
						: 'text-neutral-500'}"
				>
					{note.status}
				</span>
			{/if}
			<DropdownMenu.Root bind:open={menuOpen}>
				<DropdownMenu.Trigger
					aria-label="Note actions"
					title="Note actions"
					class="cursor-pointer rounded px-1 py-0.5 text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200"
				>
					<EllipsisIcon class="size-3.5" />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" class="w-40">
					<DropdownMenu.Item onSelect={() => onedit(note)}>
						<PencilIcon />Edit
					</DropdownMenu.Item>
					{#if canRetry}
						<DropdownMenu.Item onSelect={() => onretry(note)}>
							<RefreshCwIcon />Re-run model
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Separator />
					<DropdownMenu.Item variant="destructive" onSelect={() => ondelete(note)}>
						<Trash2Icon />Delete
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		</div>

		{#if note.tags.length > 0}
			<ul
				class="absolute right-3 bottom-2 flex gap-1.5 bg-neutral-900 pl-2 font-mono text-[0.625rem] text-neutral-500 opacity-0 transition group-focus-within:opacity-100 group-hover:opacity-100"
			>
				{#each note.tags as tag (tag)}
					<li>
						<button
							type="button"
							onclick={() => ontag(tag)}
							title="Show #{tag} notes"
							class="cursor-pointer hover:text-neutral-200">#{tag}</button
						>
					</li>
				{/each}
			</ul>
		{/if}
	{/if}
</article>
