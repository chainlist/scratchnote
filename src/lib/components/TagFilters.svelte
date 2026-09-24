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

	let root = $state<HTMLDivElement>();
	let list = $state<HTMLDivElement>();
	let open = $state(false);
	/** Chips that wrapped past the first row, so are clipped while collapsed. */
	let hidden = $state(0);

	// The row is clamped by clipping, not by removing chips, so layout is the
	// same open or closed and a chip's offsetTop says which row it landed on.
	function measure() {
		if (!list) return;
		const chips = [...list.querySelectorAll<HTMLElement>('[data-chip]')];
		const top = chips[0]?.offsetTop ?? 0;
		hidden = chips.filter((c) => c.offsetTop > top).length;
		if (hidden === 0) open = false;
	}

	$effect(() => {
		void related;
		void active;
		measure();
	});

	$effect(() => {
		if (!list) return;
		const observer = new ResizeObserver(measure);
		observer.observe(list);
		return () => observer.disconnect();
	});

	function pick(fn: (tag: string) => void, tag: string) {
		open = false;
		fn(tag);
	}

	function onpointerdown(e: PointerEvent) {
		if (open && root && !root.contains(e.target as Node)) open = false;
	}

	// Tabbing onto a clipped chip opens the panel so focus stays visible.
	function onfocusin(e: FocusEvent) {
		const first = list?.querySelector<HTMLElement>('[data-chip]');
		const target = e.target as HTMLElement;
		if (first && target.offsetTop > first.offsetTop) open = true;
	}

	function onkeydown(e: KeyboardEvent) {
		if (open && e.key === 'Escape') open = false;
	}

	const chip =
		'inline-flex h-6 shrink-0 cursor-pointer items-center gap-1 rounded-md border px-2 font-mono text-xs transition-colors';
</script>

<svelte:window {onpointerdown} {onkeydown} />

{#if active.length > 0 || related.length > 0}
	<!-- Holds one row of space; the panel inside grows over the content below. -->
	<div bind:this={root} class="relative mb-4 h-6">
		<div
			class="absolute -inset-x-2 -top-2 z-20 flex items-start gap-1.5 rounded-lg p-2 {open
				? 'bg-popover shadow-lg ring-1 ring-border'
				: ''}"
		>
			<div
				bind:this={list}
				{onfocusin}
				class="flex min-w-0 flex-1 flex-wrap items-center gap-1.5 {open
					? ''
					: 'max-h-6 overflow-hidden'}"
			>
				{#each active as tag (tag)}
					<button
						type="button"
						data-chip
						onclick={() => pick(onremove, tag)}
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
							data-chip
							onclick={() => pick(onadd, tag)}
							title="Narrow to #{tag}"
							class="{chip} border-transparent text-muted-foreground hover:border-input hover:text-foreground"
						>
							#{tag}
							<span class="text-muted-foreground/70">{count}</span>
						</button>
					{/each}
				{/if}
			</div>
			{#if hidden > 0}
				<button
					type="button"
					onclick={() => (open = !open)}
					aria-expanded={open}
					class="{chip} border-transparent text-muted-foreground hover:border-input hover:text-foreground"
				>
					{open ? 'Less' : `+${hidden} more`}
				</button>
			{/if}
		</div>
	</div>
{/if}
