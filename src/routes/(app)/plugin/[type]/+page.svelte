<script lang="ts">
	import { untrack } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { page } from '$app/state';
	import View from '#lib/components/View.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { openView, type Header } from '#lib/plugins/host.js';
	import { registry, type ViewEntry } from '#lib/plugins/registry.svelte.js';
	import { getShell } from '#lib/shell.svelte.js';

	const shell = getShell();

	const type = $derived(page.params.type ?? '');
	const entry = $derived(registry.pages.find((candidate) => candidate.type === type));
	/** The whole query string, so a page opens anew when any of it changes. */
	const search = $derived(page.url.search);

	let header = $state<Header>({ title: '', icon: '' });

	const mountPage =
		(view: ViewEntry, query: string): Attachment<HTMLElement> =>
		(el) =>
			untrack(() => {
				const params = Object.fromEntries(new URLSearchParams(query));
				return openView(view, el, params, (next) => (header = next));
			});
</script>

<!-- A plugin's page (SPEC 3.9): its title and a way back to the day, as the
     app's own pages have, and the rest is the plugin's. -->
<View
	back={shell.back}
	title={entry ? header.title : undefined}
	pin={entry && header.title
		? { kind: 'view', target: `${type}${search}`, label: header.title }
		: undefined}
	detail={header.detail}
	key={type}
	fill={entry?.fill}
>
	{#if entry}
		{#key entry}
			{#key search}
				<div class={entry.fill ? 'h-full' : undefined} {@attach mountPage(entry, search)}></div>
			{/key}
		{/key}
	{:else}
		<p class="text-base text-meta">{m.plugins_page_missing()}</p>
	{/if}
</View>
