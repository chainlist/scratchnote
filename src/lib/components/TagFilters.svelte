<script lang="ts">
	import type { Note } from '$lib/api';
	import XIcon from '@lucide/svelte/icons/x';

	let {
		active,
		results,
		onremove,
		onadd
	}: {
		/** Tags filtered on, from the query. */
		active: string[];
		/** The notes the query matches, whose other tags are offered to narrow it. */
		results: Note[];
		onremove: (tag: string) => void;
		onadd: (tag: string) => void;
	} = $props();

	/** Related tags shown at once. */
	const LIMIT = 10;

	// A tag every result already carries would not narrow anything, so only
	// tags on some of the results are offered, most common first.
	const related = $derived.by(() => {
		const counts: Record<string, number> = {};
		for (const note of results) {
			for (const tag of note.tags) {
				if (!active.includes(tag)) counts[tag] = (counts[tag] ?? 0) + 1;
			}
		}
		return Object.entries(counts)
			.filter(([, n]) => n < results.length)
			.sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
			.slice(0, LIMIT);
	});

	const chip =
		'inline-flex h-6 cursor-pointer items-center gap-1 rounded-md border px-2 font-mono text-xs transition-colors';
</script>

{#if active.length > 0 || related.length > 0}
	<div class="mb-4 flex flex-wrap items-center gap-1.5">
		{#each active as tag (tag)}
			<button
				type="button"
				onclick={() => onremove(tag)}
				aria-label="Remove #{tag} filter"
				class="{chip} border-input bg-input/30 text-foreground hover:bg-input/60"
			>
				#{tag}
				<XIcon class="size-3 text-muted-foreground" />
			</button>
		{/each}
		{#if related.length > 0}
			{#if active.length > 0}
				<span class="mx-1 h-4 w-px bg-border" aria-hidden="true"></span>
			{/if}
			{#each related as [tag, count] (tag)}
				<button
					type="button"
					onclick={() => onadd(tag)}
					title="Narrow to #{tag}"
					class="{chip} border-transparent text-muted-foreground hover:border-input hover:text-foreground"
				>
					#{tag}
					<span class="text-muted-foreground/70">{count}</span>
				</button>
			{/each}
		{/if}
	</div>
{/if}
