<script lang="ts">
	import type { Snippet } from 'svelte';
	import ViewHeader from '#lib/components/ViewHeader.svelte';
	import { getShell, type ViewTitle } from '#lib/shell.svelte.js';

	let {
		key,
		fill = false,
		children,
		...header
	}: ViewTitle & {
		/** Changes when another of the same view opens, such as the next day,
		 *  which then rises into place as a view of its own does. */
		key?: unknown;
		/** The whole main area under the title rather than the reading
		 *  column, with a set height, as a plugin's page may ask. */
		fill?: boolean;
		children: Snippet;
	} = $props();

	const shell = getShell();

	/** A view opens at its top. */
	function fromTop(node: HTMLElement) {
		node.closest('main')?.scrollTo({ top: 0 });
	}
</script>

<!-- A view's column: its title, what the app has to say, and the view. -->
<div class={fill ? 'flex h-full w-full flex-col' : 'mx-auto w-full max-w-3xl'}>
	<ViewHeader {...header} />

	{#if shell.error}
		<p
			class="rounded border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
		>
			{shell.error}
		</p>
	{/if}

	{#key key}
		<div class={['page-in', fill && 'min-h-0 flex-1']} {@attach fromTop}>
			{@render children()}
		</div>
	{/key}
</div>
