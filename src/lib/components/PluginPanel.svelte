<script lang="ts">
	import { untrack } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import Dock from '#lib/components/Dock.svelte';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import { openView, type Header } from '#lib/plugins/host.js';
	import { registry, type ViewEntry } from '#lib/plugins/registry.svelte.js';

	/** A plugin's view in the dock, where a page docks (SPEC 3.9). */
	let { type, onclose }: { type: string; onclose: () => void } = $props();

	const entry = $derived(registry.views.find((view) => view.type === type));
	let header = $state<Header>({ title: '', icon: '' });

	// The view is opened once per entry, whatever it reads as it opens; a
	// plugin switched off takes its entry away, and the panel with it.
	const mountView =
		(view: ViewEntry): Attachment<HTMLElement> =>
		(el) =>
			untrack(() => openView(view, el, {}, (next) => (header = next)));

	$effect(() => {
		if (!entry) onclose();
	});
</script>

{#if entry}
	<Dock label={header.title} {onclose}>
		{#snippet title()}
			{#if header.icon}<PluginIcon icon={header.icon} class="size-4 text-muted-foreground" />{/if}
			<h2 class="min-w-0 flex-1 truncate text-sm font-semibold">{header.title}</h2>
		{/snippet}
		{#key entry}
			<div class="min-h-0 flex-1 overflow-y-auto px-4 pb-16" {@attach mountView(entry)}></div>
		{/key}
	</Dock>
{/if}
