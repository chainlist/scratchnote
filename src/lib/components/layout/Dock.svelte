<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import PanelLeftIcon from '@lucide/svelte/icons/panel-left';
	import PanelLeftCloseIcon from '@lucide/svelte/icons/panel-left-close';
	import PanelRightIcon from '@lucide/svelte/icons/panel-right';
	import PanelRightCloseIcon from '@lucide/svelte/icons/panel-right-close';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	/** What the dock holds, a page or a plugin's view, under a row with its
	 *  title and the buttons that move it to the view's other side and close
	 *  it (SPEC 3.5). */
	let {
		label,
		title,
		onclose,
		children
	}: {
		label: string;
		title?: Snippet;
		onclose: () => void;
		children: Snippet;
	} = $props();

	const shell = getShell();

	const left = $derived(shell.dockSide === 'left');
	const move = $derived(left ? m.dock_move_right() : m.dock_move_left());
</script>

<aside aria-label={label} class="flex h-full flex-col">
	<div class="flex items-center gap-1 px-3 pt-3 pb-2">
		<div class="flex min-w-0 flex-1 items-center gap-2 px-1">{@render title?.()}</div>
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={shell.flipDock}
			aria-label={move}
			title={move}
			class="text-muted-foreground hover:text-foreground"
		>
			{#if left}<PanelRightIcon />{:else}<PanelLeftIcon />{/if}
		</Button>
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onclose}
			aria-label={m.common_close()}
			title={m.common_close()}
			class="text-muted-foreground hover:text-foreground"
		>
			{#if left}<PanelLeftCloseIcon />{:else}<PanelRightCloseIcon />{/if}
		</Button>
	</div>
	{@render children()}
</aside>
