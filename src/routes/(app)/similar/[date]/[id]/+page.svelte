<script lang="ts">
	import Markdown from '#lib/components/Markdown.svelte';
	import NoteList from '#lib/components/NoteList.svelte';
	import View from '#lib/components/layout/View.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data, params } = $props();

	const shell = getShell();
</script>

<View back={shell.back} title={m.note_similar()} key={params.id}>
	{#if data.note}
		<div class="mb-6 rounded-lg border border-neutral-800 px-3 py-2 text-sm text-neutral-400">
			<Markdown text={data.note.body} links={false} class="max-h-[3lh] overflow-hidden" />
		</div>
	{/if}
	<NoteList notes={data.similar} empty={m.page_no_similar()} showDate {...shell.cardActions} />
</View>
