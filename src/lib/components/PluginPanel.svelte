<script lang="ts">
	import { untrack } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import { Button } from '$lib/components/ui/button';
	import PanelRightCloseIcon from '@lucide/svelte/icons/panel-right-close';
	import PluginIcon from '$lib/components/PluginIcon.svelte';
	import { m } from '$lib/paraglide/messages';
	import { openView, type Header } from '$lib/plugins/host';
	import { registry, type ViewEntry } from '$lib/plugins/registry.svelte';

	/** A plugin's view docked on the right, where a page docks (SPEC 3.9). */
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
	<aside
		aria-label={header.title}
		class="flex w-(--page-dock) shrink-0 flex-col border-l border-neutral-800"
	>
		<div class="flex items-center gap-2 px-4 pt-3 pb-2">
			{#if header.icon}<PluginIcon icon={header.icon} class="size-4 text-muted-foreground" />{/if}
			<h2 class="min-w-0 flex-1 truncate text-sm font-semibold">{header.title}</h2>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={onclose}
				aria-label={m.common_close()}
				title={m.common_close()}
				class="text-muted-foreground hover:text-foreground"
			>
				<PanelRightCloseIcon />
			</Button>
		</div>
		{#key entry}
			<div class="min-h-0 flex-1 overflow-y-auto px-4 pb-6" {@attach mountView(entry)}></div>
		{/key}
	</aside>
{/if}
