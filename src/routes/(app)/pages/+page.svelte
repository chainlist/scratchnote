<script lang="ts">
	import NoteList from '#lib/components/NoteList.svelte';
	import View from '#lib/components/View.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();
</script>

<View back={shell.back} title={m.pages_all()}>
	<NoteList notes={data.pages} empty={m.pages_none()} showDate {...shell.cardActions} />
</View>
