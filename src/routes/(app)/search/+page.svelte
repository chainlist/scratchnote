<script lang="ts">
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import NoteList from '#lib/components/NoteList.svelte';
	import View from '#lib/components/layout/View.svelte';
	import ShowMore from '#lib/components/ShowMore.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { RESULTS_STEP } from '#lib/query.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();

	/** List a stretch more where the list is, without a step in the history. */
	function showMore() {
		const q = encodeURIComponent(data.query);
		// resolve() takes no query string, so the query follows the path it gives.
		void goto(`${resolve('search/')}?q=${q}&n=${data.shown + RESULTS_STEP}`, {
			replace: true,
			reset: false
		});
	}
</script>

<!-- The whole query, after the count. -->
<View
	back={shell.back}
	title={m.page_results({ count: data.found.total })}
	detail={data.query.trim()}
>
	<NoteList notes={data.found.notes} empty={m.page_no_match()} showDate {...shell.cardActions} />
	{#if data.found.notes.length < data.found.total}
		<ShowMore ghost class="mt-4" onclick={showMore} />
	{/if}
</View>
