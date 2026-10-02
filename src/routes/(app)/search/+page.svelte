<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import NoteList from '#lib/components/NoteList.svelte';
	import View from '#lib/components/View.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { RESULTS_STEP } from '#lib/query.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();

	/** List a stretch more where the list is, without a step in the history. */
	function showMore() {
		const q = encodeURIComponent(data.query);
		// resolve() takes no query string, so the query follows the path it gives.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		void goto(`${resolve('search/')}?q=${q}&n=${data.shown + RESULTS_STEP}`, {
			replace: true,
			reset: false
		});
	}
</script>

<!-- The whole query, so a `#category` filter shows too. A query refined by a
     category click stays the same view. -->
<View
	back={shell.back}
	title={m.page_results({ count: data.found.total })}
	detail={data.query.trim()}
>
	<NoteList notes={data.found.notes} empty={m.page_no_match()} showDate {...shell.cardActions} />
	{#if data.found.notes.length < data.found.total}
		<Button variant="ghost" class="mt-4 text-muted-foreground" onclick={showMore}>
			{m.search_show_more()}
		</Button>
	{/if}
</View>
