<script lang="ts">
	import type { Snippet } from 'svelte';
	import type { SpacesView } from '#lib/api.js';
	import AppUpdate from '#lib/components/AppUpdate.svelte';
	import SpaceSwitcher from '#lib/components/SpaceSwitcher.svelte';
	import WindowControls from '#lib/components/WindowControls.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import SearchIcon from '@lucide/svelte/icons/search';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import { m } from '#lib/paraglide/messages.js';

	let {
		spaces,
		onsearch,
		onsettings,
		title,
		titleShown = false
	}: {
		spaces: SpacesView | null;
		onsearch: () => void;
		onsettings: () => void;
		/** Centred in the bar, for what the page has scrolled out of view. */
		title?: Snippet;
		titleShown?: boolean;
	} = $props();

	// macOS keeps its native traffic lights over the top left corner;
	// elsewhere the window is undecorated and draws its own controls.
	const mac = navigator.userAgent.includes('Mac');
</script>

<header
	data-tauri-drag-region="deep"
	class={['relative flex h-12 shrink-0 items-center gap-1 pr-2', mac ? 'pl-20' : 'pl-4']}
>
	<SpaceSwitcher view={spaces} />
	{#if title}
		<!-- Rises into place as the page's own title leaves, and sinks back out.
		     Centred on the window, it keeps clear of the bar's two ends, the
		     wider being the window's buttons, and is cut short in a narrow one. -->
		<div
			inert={!titleShown}
			class="absolute left-1/2 flex max-w-[calc(100%-28rem)] -translate-x-1/2 items-center gap-1 transition duration-200 ease-out {titleShown
				? 'translate-y-0 opacity-100'
				: 'pointer-events-none translate-y-2 opacity-0'}"
		>
			{@render title()}
		</div>
	{/if}
	<div class="ml-auto flex items-center gap-1">
		<AppUpdate />
		<Button
			variant="ghost"
			size="sm"
			onclick={onsearch}
			aria-label={m.search_label()}
			title={m.search_label()}
			class="text-muted-foreground hover:text-foreground"
		>
			<SearchIcon />
			<kbd class="font-mono text-xs">{mac ? '⌘' : 'Ctrl'} /</kbd>
		</Button>
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onsettings}
			aria-label={m.common_settings()}
			title={m.common_settings()}
			class="text-muted-foreground hover:text-foreground"
		>
			<SettingsIcon />
		</Button>
		{#if !mac}<WindowControls />{/if}
	</div>
</header>
