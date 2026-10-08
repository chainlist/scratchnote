<script lang="ts">
	import NoteList from '#lib/components/NoteList.svelte';
	import View from '#lib/components/View.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { RESULTS_STEP } from '#lib/query.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();

	/** How many pages are drawn, a stretch at a time. */
	let drawn = $state(RESULTS_STEP);
</script>

<View back={shell.back} title={m.pages_all()}>
	<NoteList
		notes={data.pages.slice(0, drawn)}
		empty={m.pages_none()}
		showDate
		{...shell.cardActions}
	/>
	{#if drawn < data.pages.length}
		<div class="mt-8 flex justify-center">
			<Button variant="outline" size="sm" onclick={() => (drawn += RESULTS_STEP)}>
				{m.search_show_more()}
				<span class="text-xs text-muted-foreground tabular-nums">{data.pages.length - drawn}</span>
			</Button>
		</div>
	{/if}
</View>
