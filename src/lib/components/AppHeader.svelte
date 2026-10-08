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
	// Android shows the app full screen, with no window to control.
	const android = navigator.userAgent.includes('Android');
</script>

<header
	data-tauri-drag-region="deep"
	class={['@container relative flex h-12 shrink-0 items-center gap-1 pr-2', mac ? 'pl-20' : 'pl-4']}
>
	<SpaceSwitcher view={spaces} />
	{#if title}
		<!-- Rises into place as the page's own title leaves, and sinks back out.
		     Centred on the window, it keeps clear of the bar's two ends, the
		     wider being the window's buttons, and is cut short in a narrow one.
		     In a narrower one still it stays away rather than cover them. -->
		<div
			inert={!titleShown}
			class="absolute left-1/2 flex max-w-[calc(100%-28rem)] min-w-0 -translate-x-1/2 items-center gap-1 overflow-hidden transition duration-200 ease-out @max-[42rem]:hidden {titleShown
				? 'translate-y-0 opacity-100'
				: 'pointer-events-none translate-y-2 opacity-0 motion-reduce:translate-y-0'}"
		>
			{@render title()}
		</div>
	{/if}
	<!-- The window's buttons never leave the bar: in a narrow one the others
	     keep only their icons. -->
	<div class="ml-auto flex shrink-0 items-center gap-1">
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
			<kbd class="font-mono text-xs @max-[36rem]:hidden">{mac ? '⌘' : 'Ctrl'} /</kbd>
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
		{#if !mac && !android}<WindowControls />{/if}
	</div>
</header>
