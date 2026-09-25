<script lang="ts">
	import type { SpacesView } from '$lib/api';
	import SpaceSwitcher from '$lib/components/SpaceSwitcher.svelte';
	import WindowControls from '$lib/components/WindowControls.svelte';
	import { Button } from '$lib/components/ui/button';
	import SearchIcon from '@lucide/svelte/icons/search';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import { m } from '$lib/paraglide/messages';

	let {
		spaces,
		onsearch,
		onsettings
	}: {
		spaces: SpacesView | null;
		onsearch: () => void;
		onsettings: () => void;
	} = $props();

	// macOS keeps its native traffic lights over the top left corner;
	// elsewhere the window is undecorated and draws its own controls.
	const mac = navigator.userAgent.includes('Mac');
</script>

<header
	data-tauri-drag-region="deep"
	class={['flex h-12 shrink-0 items-center gap-1 pr-2', mac ? 'pl-20' : 'pl-4']}
>
	<SpaceSwitcher view={spaces} />
	<div class="ml-auto flex items-center gap-1">
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
