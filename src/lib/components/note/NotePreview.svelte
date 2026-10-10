<script lang="ts">
	import { isPage, type Note } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { m } from '#lib/paraglide/messages.js';

	let {
		note,
		clamp
	}: {
		note: Note;
		/** How much of the text shows, such as `line-clamp-2`. */
		clamp: string;
	} = $props();
</script>

<!-- The note or page a dialog acts on, its title first for a page. -->
<div class="rounded-lg border bg-muted/40 px-3 py-2 text-sm text-muted-foreground">
	{#if isPage(note)}
		<p class="truncate font-medium text-foreground">{note.subject ?? m.pages_untitled()}</p>
	{/if}
	{#if note.body}
		<Markdown text={note.body} links={false} class={clamp} />
	{/if}
</div>
