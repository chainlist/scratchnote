<script lang="ts">
	import SearchIcon from '@lucide/svelte/icons/search';
	import type { Note } from '#lib/api.js';
	import * as InputGroup from '#lib/components/ui/input-group/index.js';
	import { shortDay } from '#lib/dates.js';
	import { noteTitle } from '#lib/markdown.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		query = $bindable(''),
		found,
		count,
		shown,
		previews,
		dayOf,
		selectedId,
		onpick
	}: {
		/** What the search box holds. */
		query?: string;
		/** The notes whose text holds its words, null while it is empty. */
		found: Set<string> | null;
		/** How many notes the search and the rest let through. */
		count: number;
		/** The first of them, listed under the box. */
		shown: string[];
		/** The notes' text as read so far, null while it is read. */
		previews: ReadonlyMap<string, Note | null>;
		/** A note's day. */
		dayOf: (id: string) => string;
		selectedId: string | null;
		/** Open a note's card on the map. */
		onpick: (id: string) => void;
	} = $props();
</script>

<div class="flex flex-col gap-1">
	<InputGroup.Root class="h-8">
		<InputGroup.Addon><SearchIcon /></InputGroup.Addon>
		<InputGroup.Input
			bind:value={query}
			placeholder={m.map_find()}
			aria-label={m.map_find()}
			onkeydown={(event) => {
				if (event.key === 'Escape' && query) {
					event.preventDefault();
					query = '';
				} else if (event.key === 'Enter' && shown[0]) {
					event.preventDefault();
					onpick(shown[0]);
				}
			}}
		/>
	</InputGroup.Root>
	<!-- Kept in the page while empty, so its first count is read out too. -->
	<p class="px-1 text-xs text-muted-foreground empty:sr-only" aria-live="polite">
		{#if found}
			{count ? m.page_results({ count }) : m.page_no_match()}
		{/if}
	</p>
	{#if shown.length}
		<!-- The first notes found, each opening its card on the map. -->
		<ul class="-mx-1 flex flex-col" aria-label={m.map_found()}>
			{#each shown as id (id)}
				{@const note = previews.get(id)}
				<li>
					<button
						type="button"
						onclick={() => onpick(id)}
						aria-current={selectedId === id ? 'true' : undefined}
						class={[
							'flex w-full min-w-0 items-baseline gap-2 rounded-md px-1 py-1 text-left text-sm transition-colors outline-none hover:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring focus-visible:outline-solid',
							selectedId === id && 'bg-muted'
						]}
					>
						<span class="shrink-0 text-xs text-muted-foreground tabular-nums">
							{shortDay(dayOf(id))}
						</span>
						<span class="min-w-0 flex-1 truncate">{note ? noteTitle(note) : ''}</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>
