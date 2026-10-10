<script lang="ts">
	import { tick, untrack } from 'svelte';
	import FileTextIcon from '@lucide/svelte/icons/file-text';
	import PencilLineIcon from '@lucide/svelte/icons/pencil-line';
	import PlusIcon from '@lucide/svelte/icons/plus';
	import XIcon from '@lucide/svelte/icons/x';
	import NoteEditor from '#lib/components/note/NoteEditor.svelte';
	import { noteDrafts } from '#lib/notes/note-draft.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		draftKey,
		onsave,
		onpage,
		onerror,
		centered = false,
		writing = $bindable(false)
	}: {
		/** The space and day the note is written on, which its draft is kept
		 *  under while the view is left. */
		draftKey: string;
		/** Save the note to the day shown. Resolves to null once saved, or to
		 *  the error as it came, shown under the editor, which stays open. */
		onsave: (body: string) => Promise<string | null>;
		/** Start a page on the day shown instead (SPEC 3.5). */
		onpage: () => void;
		/** A file could not be attached. */
		onerror: (message: string) => void;
		/** On an empty day, with no cards to line up under: the button sits centred and the editor spans the page. */
		centered?: boolean;
		/** The editor is open rather than the button. */
		writing?: boolean;
	} = $props();

	let saving = $state(false);
	/** The last save's error, until a save works. */
	let failure = $state<string>();
	// What was written here before the view was left, open as it was then.
	const kept = untrack(() => noteDrafts.get(draftKey));
	let draft = $state(kept?.body ?? '');
	if (kept?.writing) writing = true;
	let editor = $state<NoteEditor | null>(null);
	let addButton = $state<HTMLButtonElement>();

	// Kept as it is typed, so that the view left by any way finds it again.
	$effect(() => {
		if (draft.trim()) noteDrafts.set(draftKey, { body: draft, writing });
		else noteDrafts.delete(draftKey);
	});

	/** The first words of a draft set aside, without the markup that starts its line. */
	const excerpt = $derived.by(() => {
		const line = draft.trim().split('\n')[0];
		return line.replace(/^(#+|>|[-*+](\s+\[[ xX]\])?)\s*/, '') || line;
	});

	/** Open the editor, or bring the focus back to it; the day's N key does too. */
	export async function start() {
		writing = true;
		await tick();
		editor?.focus();
		// The editor opens under the last note, often below the window's edge.
		editor?.reveal();
	}

	async function discard() {
		draft = '';
		await tick();
		addButton?.focus();
	}

	async function save() {
		if (saving) return;
		if (draft.trim() === '') {
			writing = false;
			return;
		}
		saving = true;
		failure = (await onsave(draft)) ?? undefined;
		saving = false;
		if (failure === undefined) {
			draft = '';
			writing = false;
		}
	}

	/** Close the editor, the draft kept, and give the focus back to the
	 *  button that opens it again. */
	async function close() {
		writing = false;
		await tick();
		addButton?.focus();
	}
</script>

<!-- Laid out like a note card so the button and the editor sit in the body
     column, under the notes above. -->
<div class={centered ? 'w-full' : '-mx-3 grid grid-cols-[4.5rem_1fr] gap-x-6 px-3 py-3'}>
	<div
		class={centered
			? writing
				? 'text-left'
				: 'flex justify-center'
			: 'col-start-2 max-w-[70ch] min-w-0'}
	>
		{#if writing}
			<NoteEditor
				bind:this={editor}
				bind:value={draft}
				placeholder={m.capture_placeholder()}
				{saving}
				{failure}
				onsave={() => void save()}
				onclose={() => void close()}
				{onerror}
			/>
		{:else}
			<!-- Wraps on a phone, where the draft's button would otherwise
			     shrink under its own label. -->
			<div
				class="flex min-w-0 flex-wrap items-center gap-x-3 gap-y-1 {centered
					? 'justify-center'
					: ''}"
			>
				<!-- A draft set aside comes back by its first words, so it is
				     never mistaken for lost. -->
				<button
					bind:this={addButton}
					type="button"
					onclick={start}
					aria-keyshortcuts="N"
					title="{excerpt ? m.note_draft_continue() : m.page_add_note()} (N)"
					class="-mx-1.5 flex min-w-0 cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 text-sm text-meta hover:bg-neutral-900 hover:text-neutral-300 focus-visible:bg-neutral-900 focus-visible:text-neutral-300"
				>
					{#if excerpt}
						<PencilLineIcon class="size-3.5 shrink-0" />
						<span class="shrink-0">{m.note_draft_continue()}</span>
						<span class="min-w-0 truncate italic">{excerpt}</span>
					{:else}
						<PlusIcon class="size-3.5" />{m.page_add_note()}
					{/if}
				</button>
				{#if excerpt}
					<button
						type="button"
						onclick={discard}
						aria-label={m.note_draft_discard_title()}
						title={m.note_draft_discard_title()}
						class="flex shrink-0 cursor-pointer items-center gap-1 rounded px-1.5 py-0.5 text-sm text-meta hover:bg-neutral-900 hover:text-neutral-300 focus-visible:bg-neutral-900 focus-visible:text-neutral-300"
					>
						<XIcon class="size-3.5" />{m.note_draft_discard()}
					</button>
				{/if}
				<button
					type="button"
					onclick={onpage}
					class="flex shrink-0 cursor-pointer items-center gap-1.5 rounded px-1.5 py-0.5 text-sm text-meta hover:bg-neutral-900 hover:text-neutral-300 focus-visible:bg-neutral-900 focus-visible:text-neutral-300"
				>
					<FileTextIcon class="size-3.5" />{m.pages_new()}
				</button>
			</div>
		{/if}
	</div>
</div>
