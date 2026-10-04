<script lang="ts">
	import { untrack } from 'svelte';
	import { afterNavigate, goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import type { Note } from '#lib/api.js';
	import PageView from '#lib/components/PageView.svelte';
	import View from '#lib/components/View.svelte';
	import { dayHeading } from '#lib/components/ViewHeader.svelte';
	import { getShell } from '#lib/shell.svelte.js';

	let { params } = $props();

	const shell = getShell();

	/** The day a new page goes on, or the page's own. */
	const date = $derived(params.date);
	/** `new` is a page not written yet; the others are ULIDs. */
	const id = $derived(params.id === 'new' ? null : params.id);

	// The page's day is the one the other views go back to, and where a
	// new page opened from here goes.
	$effect(() => {
		shell.day = date;
	});

	/** The page the view shows: null while it is a new one, then the id its
	 *  first save gives it. Only another page opens a view of its own; a New
	 *  page asked for while one is open is that one. */
	let shown = untrack(() => id);
	/** Counts the pages opened here, each of which rises into place. */
	let opened = $state(0);

	afterNavigate(({ shallow }) => {
		if (shallow) return;
		if (id === shown) return;
		shown = id;
		opened++;
	});

	/** A new page has its title and its file; the view stays as it is. */
	async function adopt(created: Note) {
		shown = created.id;
		await goto(resolve(`page/${created.date}/${created.id}/`), {
			replace: true,
			reset: false
		});
		await shell.refresh();
	}

	let view = $state<PageView>();

	// An open new page takes the capture window's text where it is typed.
	$effect(() => {
		if (id !== null || !view) return;
		const add = view.addText;
		shell.draft = add;
		return () => {
			if (shell.draft === add) shell.draft = null;
		};
	});
</script>

<View back={resolve(`day/${date}/`)} key={opened}>
	{#snippet heading(compact: boolean)}
		<!-- The page's title is its own heading, in the view; this is its day. -->
		<span class="text-sm font-medium whitespace-nowrap text-muted-foreground">
			{dayHeading(date, compact)}
		</span>
	{/snippet}

	<PageView
		bind:this={view}
		{id}
		{date}
		oncreated={(created) => void adopt(created)}
		ondelete={(note) => (shell.deleting = note)}
		onsimilar={shell.canSimilar ? shell.showSimilar : undefined}
		onmove={shell.canMove ? shell.askMove : undefined}
		onopennote={(note) => void shell.openCited(note)}
	/>
</View>
