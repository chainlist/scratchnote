<script lang="ts">
	import type { Note } from '$lib/api';

	let { note }: { note: Note } = $props();
</script>

<article class="rounded-lg border border-neutral-800 bg-neutral-900 p-4">
	<header class="mb-1 flex items-baseline gap-3">
		<time class="font-mono text-xs text-neutral-500">{note.time}</time>
		<h3 class="flex-1 text-sm font-medium text-neutral-100">
			{note.subject ?? 'Untitled'}
		</h3>
		{#if note.status !== 'done'}
			<span
				class="rounded px-1.5 py-0.5 text-[10px] tracking-wide uppercase
					{note.status === 'failed' ? 'bg-red-950 text-red-400' : 'bg-neutral-800 text-neutral-400'}"
			>
				{note.status}
			</span>
		{/if}
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
