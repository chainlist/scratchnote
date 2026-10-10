<script lang="ts">
	import NoteList from '#lib/components/note/NoteList.svelte';
	import View from '#lib/components/layout/View.svelte';
	import ShowMore from '#lib/components/common/ShowMore.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { RESULTS_STEP } from '#lib/notes/query.js';
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
		<ShowMore
			class="mt-8"
			count={data.pages.length - drawn}
			onclick={() => (drawn += RESULTS_STEP)}
		/>
	{/if}
</View>
