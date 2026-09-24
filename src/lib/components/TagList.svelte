<script lang="ts">
	import { sidebarItem } from '$lib/components/sidebar';
	import XIcon from '@lucide/svelte/icons/x';

	let {
		tags,
		active,
		onselect
	}: {
		tags: [string, number][];
		/** Tags currently filtered on, highlighted in the list. */
		active: string[];
		/** `additive` is true when Ctrl (Cmd on macOS) is held. */
		onselect: (tag: string, additive: boolean) => void;
	} = $props();

	/** Rows shown before "Show all", so the calendar keeps its room. */
	const TOP = 12;

	let filter = $state('');
	let expanded = $state(false);

	const needle = $derived(filter.trim().replace(/^#+/, '').toLowerCase());

	// Active tags sit in their own group above, so the list below skips them.
	const rest = $derived(tags.filter(([tag]) => !active.includes(tag)));

	// Tags arrive most used first. A filter keeps that order but puts names
	// starting with the text ahead of names that merely contain it.
	const matches = $derived.by(() => {
		if (!needle) return rest;
		const starts: [string, number][] = [];
		const contains: [string, number][] = [];
		for (const entry of rest) {
			const name = entry[0].toLowerCase();
			if (name.startsWith(needle)) starts.push(entry);
			else if (name.includes(needle)) contains.push(entry);
		}
		return [...starts, ...contains];
	});

	const shown = $derived(needle || expanded ? matches : matches.slice(0, TOP));
	const counts = $derived(new Map(tags));

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Escape') {
			filter = '';
		} else if (event.key === 'Enter' && matches.length > 0) {
			onselect(matches[0][0], event.ctrlKey || event.metaKey);
			filter = '';
		}
	}
</script>

{#if tags.length > 0}
	<!-- Fills what the sidebar has left; only the list scrolls, the heading stays. -->
	<section class="flex min-h-0 flex-1 flex-col">
		<h2
			class="mb-1 flex items-baseline justify-between px-2 text-[0.6875rem] font-medium tracking-wide text-muted-foreground uppercase"
		>
			Tags
			<span class="font-mono tracking-normal">{tags.length}</span>
		</h2>

		<input
			type="search"
			bind:value={filter}
			{onkeydown}
			placeholder="Filter tags"
			aria-label="Filter tags"
			spellcheck="false"
			class="mb-1 h-7 w-full shrink-0 rounded-md border border-input bg-transparent px-2 text-sm outline-none placeholder:text-muted-foreground focus-visible:border-ring focus-visible:ring-[3px] focus-visible:ring-ring/50 dark:bg-input/30 [&::-webkit-search-cancel-button]:appearance-none"
		/>

		{#if active.length > 0}
			<ul class="mb-1 flex shrink-0 flex-col gap-0.5 border-b pb-1">
				{#each active as tag (tag)}
					<li>
						<button
							type="button"
							onclick={(event) => onselect(tag, event.ctrlKey || event.metaKey)}
							title="Remove filter"
							class="{sidebarItem(true)} group justify-between"
						>
							<span class="truncate">#{tag}</span>
							<span class="font-mono text-xs text-muted-foreground group-hover:hidden">
								{counts.get(tag) ?? 0}
							</span>
							<XIcon class="hidden text-muted-foreground group-hover:block" />
						</button>
					</li>
				{/each}
			</ul>
		{/if}

		<ul class="flex min-h-0 flex-1 flex-col gap-0.5 overflow-y-auto">
			{#each shown as [tag, count] (tag)}
				<li>
					<button
						type="button"
						onclick={(event) => onselect(tag, event.ctrlKey || event.metaKey)}
						class="{sidebarItem(false)} justify-between"
					>
						<span class="truncate">#{tag}</span>
						<span class="font-mono text-xs text-muted-foreground">{count}</span>
					</button>
				</li>
			{:else}
				{#if needle}
					<li class="px-2 py-1 text-sm text-muted-foreground">No tags match</li>
				{/if}
			{/each}
			{#if !needle && matches.length > TOP}
				<li>
					<button
						type="button"
						onclick={() => (expanded = !expanded)}
						class="{sidebarItem(false)} text-xs"
					>
						{expanded ? 'Show less' : `Show all ${matches.length}`}
					</button>
				</li>
			{/if}
		</ul>
	</section>
{/if}
