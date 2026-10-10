<script lang="ts">
	import PinIcon from '@lucide/svelte/icons/pin';
	import type { Pin } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		pin,
		hoverOnly = false,
		class: className
	}: {
		/** What the button pins to the left edge, or takes off it. */
		pin: Pin;
		/** Shown only while its row is pointed at or it has the focus, unless pinned. */
		hoverOnly?: boolean;
		class?: string;
	} = $props();

	const shell = getShell();
	const pinned = $derived(shell.isPinned(pin));
</script>

<Button
	variant="ghost"
	size="icon-sm"
	onclick={() => void shell.togglePin(pin)}
	aria-label={m.pin_add()}
	aria-pressed={pinned}
	title={pinned ? m.pin_remove() : m.pin_add()}
	class={[
		'shrink-0',
		className,
		pinned
			? 'text-foreground'
			: hoverOnly
				? 'text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-foreground focus-visible:opacity-100'
				: 'text-muted-foreground hover:text-foreground'
	]}
>
	<PinIcon class={pinned ? 'fill-current' : undefined} />
</Button>
