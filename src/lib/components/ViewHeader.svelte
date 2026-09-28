<script lang="ts" module>
	import { getLocale } from '$lib/paraglide/runtime';

	/** A day as its title reads, or shorter, as the top bar has it. */
	export function dayHeading(date: string, short = false) {
		return new Date(`${date}T00:00:00`).toLocaleDateString(
			getLocale(),
			short
				? { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' }
				: { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }
		);
	}
</script>

<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import { m } from '$lib/paraglide/messages';
	import { getShell, type ViewTitle } from '$lib/shell.svelte';

	let {
		back,
		title,
		detail,
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
		const mine: ViewTitle = { back, title, detail, heading };
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
			class={compact
				? 'min-w-0 truncate text-sm font-semibold'
				: 'min-w-0 truncate text-2xl font-semibold tracking-tight'}
		>
			{title}
			{#if detail}<span class="font-normal text-muted-foreground">{detail}</span>{/if}
		</svelte:element>
	{/if}
</div>
