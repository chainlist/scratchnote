<script lang="ts">
	import type { Note } from '$lib/api';

	let { note, ondelete }: { note: Note; ondelete: (note: Note) => void } = $props();

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
		<time class="font-mono text-xs text-neutral-500">{note.time}</time>
		<h3 class="flex-1 text-sm font-medium text-neutral-100">
			{note.subject ?? 'Untitled'}
		</h3>
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

	{#if note.summary}
		<p class="mb-2 text-xs text-neutral-400">{note.summary}</p>
	{/if}

	{#if note.tags.length > 0}
		<ul class="mb-2 flex flex-wrap gap-1">
			{#each note.tags as tag (tag)}
				<li class="rounded bg-neutral-800 px-1.5 py-0.5 text-[11px] text-neutral-400">
					#{tag}
				</li>
			{/each}
		</ul>
	{/if}

	<p class="text-sm leading-relaxed whitespace-pre-wrap text-neutral-300">{note.body}</p>
</article>
