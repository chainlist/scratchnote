<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import PinIcon from '@lucide/svelte/icons/pin';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell, type ViewTitle } from '#lib/shell.svelte.js';

	let {
		back,
		title,
		pin,
		detail,
		tentative = false,
		heading,
		compact = false
	}: ViewTitle & {
		/** The small copy the top bar shows in place of the scrolled-away title. */
		compact?: boolean;
	} = $props();

	const shell = getShell();

	let row = $state<HTMLElement>();

	// The top bar shows a copy of this title while it is scrolled away.
	$effect(() => {
		if (compact) return;
		const mine: ViewTitle = { back, title, pin, detail, tentative, heading };
		shell.title = mine;
		return () => {
			if (shell.title === mine) shell.title = null;
		};
	});

	// The page scrolls inside its own pane, which clips the title as it goes
	// under the bar, so any clipping at all means it is on its way out.
	$effect(() => {
		if (!row || compact) return;
		const observer = new IntersectionObserver(
			([entry]) => (shell.titleCollapsed = entry.intersectionRatio < 1),
			{ threshold: 1 }
		);
		observer.observe(row);
		return () => {
			observer.disconnect();
			shell.titleCollapsed = false;
		};
	});
</script>

<div
	bind:this={row}
	class={compact ? 'flex max-w-sm min-w-0 items-center gap-1' : 'mt-6 mb-8 flex items-center gap-2'}
>
	{#if back}
		<Button
			variant="ghost"
			size="icon-sm"
			href={back}
			onclick={shell.goBack}
			aria-label={m.page_back()}
			title={m.page_back()}
			class="text-muted-foreground hover:text-foreground"
		>
			<ArrowLeftIcon />
		</Button>
	{/if}
	{#if heading}
		{@render heading(compact)}
	{:else if title}
		<!-- The page has its heading; the copy in the bar is not a second one. -->
		<svelte:element
			this={compact ? 'span' : 'h1'}
			class={[
				compact
					? 'min-w-0 truncate text-sm font-semibold'
					: 'min-w-0 truncate text-2xl font-semibold tracking-tight',
				tentative && 'font-normal! text-neutral-300'
			]}
		>
			{title}
			{#if detail}<span class="font-normal text-muted-foreground">{detail}</span>{/if}
		</svelte:element>
	{/if}
	{#if pin && !compact}
		{@const pinned = shell.isPinned(pin)}
		{@const label = pinned ? m.pin_remove() : m.pin_add()}
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={() => void shell.togglePin(pin)}
			aria-label={m.pin_add()}
			aria-pressed={pinned}
			title={label}
			class={[
				'shrink-0',
				pinned ? 'text-foreground' : 'text-muted-foreground hover:text-foreground'
			]}
		>
			<PinIcon class={pinned ? 'fill-current' : undefined} />
		</Button>
	{/if}
</div>
