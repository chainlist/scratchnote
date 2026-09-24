<script lang="ts">
	import type { Note, NoteEdit } from '$lib/api';

	let {
		note,
		ondelete,
		onsave,
		onretry,
		showDate = false
	}: {
		note: Note;
		ondelete: (note: Note) => void;
		/** Resolves true once saved; false keeps the editor open. */
		onsave: (note: Note, edit: NoteEdit) => Promise<boolean>;
		onretry: (note: Note) => void;
		/** Search results span days, so each card says which one. */
		showDate?: boolean;
	} = $props();

	let editing = $state(false);
	let saving = $state(false);
	let draft = $state('');

	// Re-running a manual note would be skipped anyway, and a pending one is
	// already in the queue.
	let canRetry = $derived(note.status === 'done' || note.status === 'failed');

	function startEditing() {
		draft = note.body;
		editing = true;
	}

	async function save() {
		if (saving) return;
		saving = true;
		// Subject and tags only feed search, so they are handed back unchanged
		// and stay with the model.
		const saved = await onsave(note, {
			body: draft,
			subject: note.subject ?? '',
			tags: note.tags
		});
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

	// The status chip is also the delete affordance: clicking it swaps the
	// label to Delete, and clicking again removes the note. Deleting rewrites
	// the day file and there is no undo, so it takes two clicks.
	let armed = $state(false);

	// A done note has no status chip to click, so it gets a quiet placeholder
	// that only appears on hover. Nothing is done before enrichment lands.
	let placeholder = $derived(note.status === 'done');
	let label = $derived(armed ? 'Delete' : placeholder ? '⋯' : note.status);
	let name = $derived(
		armed ? 'Confirm delete' : placeholder ? 'Delete note' : `${note.status}, click to delete`
	);

	function onChipClick() {
		if (armed) ondelete(note);
		else armed = true;
	}
</script>

<article
	class="group relative -mx-3 grid grid-cols-[4.5rem_1fr] gap-x-8 rounded-lg px-3 py-4 transition-colors duration-300 ease-out focus-within:bg-neutral-900 hover:bg-neutral-900"
	onmouseleave={() => (armed = false)}
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
					bind:value={draft}
					rows={Math.min(16, Math.max(3, draft.split('\n').length))}
					aria-label="Body"
					spellcheck="false"
					class="-mx-2 w-[calc(100%+1rem)] resize-y rounded border border-neutral-800 bg-neutral-900 px-2 py-1 text-[0.9375rem] leading-7 text-neutral-100 focus:border-neutral-600 focus:outline-none"
				></textarea>
				<div class="flex justify-end gap-1">
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
			class="absolute top-3 right-3 flex gap-1 bg-neutral-900 pl-2 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100"
		>
			{#if canRetry}
				<button type="button" onclick={() => onretry(note)} class={action}>Re-run</button>
			{/if}
			<button type="button" onclick={startEditing} class={action}>Edit</button>
			<button
				type="button"
				onclick={onChipClick}
				onblur={() => (armed = false)}
				aria-label={name}
				title={name}
				class="cursor-pointer rounded px-1.5 py-0.5 text-[0.625rem] tracking-wide uppercase
					{armed
					? 'bg-red-600 text-white dark:bg-red-900 dark:text-red-100'
					: note.status === 'failed'
						? 'bg-red-50 text-red-600 dark:bg-red-950 dark:text-red-400'
						: 'text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200'}"
			>
				{label}
			</button>
		</div>

		{#if note.tags.length > 0}
			<ul
				class="pointer-events-none absolute right-3 bottom-2 flex gap-1.5 bg-neutral-900 pl-2 font-mono text-[0.625rem] text-neutral-500 opacity-0 transition group-focus-within:opacity-100 group-hover:opacity-100"
			>
				{#each note.tags as tag (tag)}
					<li>#{tag}</li>
				{/each}
			</ul>
		{/if}
	{/if}
</article>
