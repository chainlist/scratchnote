<script lang="ts">
	import { onMount } from 'svelte';
	import { getCurrentWindow } from '@tauri-apps/api/window';
	import MinusIcon from '@lucide/svelte/icons/minus';
	import SquareIcon from '@lucide/svelte/icons/square';
	import CopyIcon from '@lucide/svelte/icons/copy';
	import XIcon from '@lucide/svelte/icons/x';
	import { cn } from '$lib/utils';
	import { m } from '$lib/paraglide/messages';

	const appWindow = getCurrentWindow();
	let maximized = $state(false);

	onMount(() => {
		const sync = async () => (maximized = await appWindow.isMaximized());
		void sync();
		const off = appWindow.onResized(() => void sync());
		return () => void off.then((stop) => stop());
	});

	const button =
		'flex size-7 items-center justify-center rounded-md text-muted-foreground transition-colors outline-none hover:bg-input/30 hover:text-foreground focus-visible:ring-[3px] focus-visible:ring-ring/50 [&_svg]:size-3.5';
</script>

<div class="flex items-center gap-1">
	<button
		class={button}
		onclick={() => appWindow.minimize()}
		aria-label={m.window_minimize()}
		title={m.window_minimize()}
	>
		<MinusIcon />
	</button>
	<button
		class={button}
		onclick={() => appWindow.toggleMaximize()}
		aria-label={maximized ? m.window_restore() : m.window_maximize()}
		title={maximized ? m.window_restore() : m.window_maximize()}
	>
		{#if maximized}<CopyIcon class="-scale-x-100" />{:else}<SquareIcon />{/if}
	</button>
	<!-- Closing only hides the window: the app keeps running in the tray. -->
	<button
		class={cn(button, 'hover:bg-red-600 hover:text-white')}
		onclick={() => appWindow.close()}
		aria-label={m.window_close()}
		title={m.window_close()}
	>
		<XIcon />
	</button>
</div>
