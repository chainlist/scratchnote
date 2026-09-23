<script lang="ts">
	import type { Note, NoteEdit } from '$lib/api';

	let {
		note,
		ondelete,
		ontag,
		onsave,
		onretry,
		showDate = false
	}: {
		note: Note;
		ondelete: (note: Note) => void;
		ontag: (tag: string) => void;
		/** Resolves true once saved; false keeps the editor open. */
		onsave: (note: Note, edit: NoteEdit) => Promise<boolean>;
		onretry: (note: Note) => void;
		/** Search results span days, so each card says which one. */
		showDate?: boolean;
	} = $props();

	let editing = $state(false);
	let saving = $state(false);
	let draft = $state<{ body: string; subject: string; tags: string }>({
		body: '',
		subject: '',
		tags: ''
	});

	// Re-running a manual note would be skipped anyway, and a pending one is
	// already in the queue.
	let canRetry = $derived(note.status === 'done' || note.status === 'failed');

	function startEditing() {
		draft = {
			body: note.body,
			subject: note.subject ?? '',
			tags: note.tags.map((tag) => `#${tag}`).join(' ')
		};
		editing = true;
	}

	async function save() {
		if (saving) return;
		saving = true;
		const saved = await onsave(note, {
			body: draft.body,
			subject: draft.subject,
			tags: draft.tags.split(/[\s,]+/).filter(Boolean)
		});
		saving = false;
		if (saved) editing = false;
	}

	const action =
		'cursor-pointer rounded px-1.5 py-0.5 text-[10px] tracking-wide text-neutral-400 uppercase hover:bg-neutral-800 hover:text-neutral-200 disabled:opacity-50';
	const field =
		'w-full rounded border border-neutral-800 bg-neutral-950 px-2 py-1 text-sm text-neutral-100 placeholder:text-neutral-600 focus:border-neutral-600 focus:outline-none';

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
	class="group rounded-lg border border-neutral-800 bg-neutral-900 p-4"
	onmouseleave={() => (armed = false)}
>
	<header class="mb-1 flex items-baseline gap-3">
		<time class="font-mono text-xs text-neutral-500">
			{#if showDate}<span class="mr-1">{note.date}</span>{/if}{note.time}
		</time>
		<h3 class="flex-1 text-sm font-medium text-neutral-100">
			{note.subject ?? 'Untitled'}
		</h3>
		{#if !editing}
			<div class="flex gap-1 opacity-0 transition group-hover:opacity-100 focus-within:opacity-100">
				{#if canRetry}
					<button type="button" onclick={() => onretry(note)} class={action}>Re-run</button>
				{/if}
				<button type="button" onclick={startEditing} class={action}>Edit</button>
			</div>
		{/if}
		<button
			type="button"
			onclick={onChipClick}
			onblur={() => (armed = false)}
			aria-label={name}
			title={name}
			class="cursor-pointer rounded px-1.5 py-0.5 text-[10px] tracking-wide uppercase
				ring-neutral-600 transition group-hover:ring-1 focus-visible:ring-1
				{armed
				? 'bg-red-900 text-red-100'
				: note.status === 'failed'
					? 'bg-red-950 text-red-400'
					: 'bg-neutral-800 text-neutral-400'}
				{placeholder && !armed ? 'opacity-0 group-hover:opacity-100 focus-visible:opacity-100' : ''}"
		>
			{label}
		</button>
	</header>

	{#if editing}
		<!-- svelte-ignore a11y_no_static_element_interactions -->
		<div class="flex flex-col gap-2" onkeydown={onEditKeydown}>
			<input
				bind:value={draft.subject}
				placeholder="Subject"
				aria-label="Subject"
				spellcheck="false"
				class={field}
			/>
			<input
				bind:value={draft.tags}
				placeholder="#tags separated by spaces"
				aria-label="Tags"
				spellcheck="false"
				class="{field} font-mono text-xs"
			/>
			<textarea
				bind:value={draft.body}
				rows={Math.min(12, Math.max(3, draft.body.split('\n').length))}
				aria-label="Body"
				spellcheck="false"
				class="{field} resize-y leading-relaxed"></textarea>
			<div class="flex items-center justify-between text-[11px] text-neutral-500">
				<span>Subject or tags edited here are never overwritten by the model.</span>
				<span class="flex gap-1">
					<button type="button" onclick={() => (editing = false)} class={action}>Cancel</button>
					<button type="button" onclick={save} disabled={saving} class={action}>
						{saving ? 'Saving' : 'Save'}
					</button>
				</span>
			</div>
		</div>
	{:else}
		{@render view()}
	{/if}
</article>

{#snippet view()}
	{#if note.summary}
		<p class="mb-2 text-xs text-neutral-400">{note.summary}</p>
	{/if}

	{#if note.tags.length > 0}
		<ul class="mb-2 flex flex-wrap gap-1">
			{#each note.tags as tag (tag)}
				<li>
					<button
						type="button"
						onclick={() => ontag(tag)}
						class="cursor-pointer rounded bg-neutral-800 px-1.5 py-0.5 text-[11px] text-neutral-400 hover:bg-neutral-700 hover:text-neutral-200"
					>
						#{tag}
					</button>
				</li>
			{/each}
		</ul>
	{/if}

	<p class="text-sm leading-relaxed whitespace-pre-wrap text-neutral-300">{note.body}</p>
{/snippet}
