<script lang="ts">
	import NoteList from '$lib/components/NoteList.svelte';
	import View from '$lib/components/View.svelte';
	import { m } from '$lib/paraglide/messages';
	import { getShell } from '$lib/shell.svelte';

	let { data } = $props();

	const shell = getShell();
</script>

<!-- The whole query, so a `#category` filter shows too. A query refined by a
     category click stays the same view. -->
<View
	back={shell.back}
	title={m.page_results({ count: data.results.length })}
	detail={data.query.trim()}
>
	<NoteList notes={data.results} empty={m.page_no_match()} showDate {...shell.cardActions} />
</View>
