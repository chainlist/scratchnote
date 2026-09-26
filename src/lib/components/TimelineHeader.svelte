<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import { queryWords } from '$lib/query';
	import type { Timeline } from '$lib/timeline';

	let {
		timeline,
		selected,
		tagCount,
		resultCount,
		onback,
		oncalendar,
		oncollapse,
		compact = false
	}: {
		timeline: Timeline;
		/** The day shown, and the one to go back to. */
		selected: string;
		tagCount: number;
		resultCount: number;
		onback: () => void;
		oncalendar: () => void;
		/** Told when the title starts under the top bar, which then shows it instead, and when it is back. */
		oncollapse?: (collapsed: boolean) => void;
		/** The small copy the top bar shows in place of the scrolled-away title. */
		compact?: boolean;
	} = $props();

	let row = $state<HTMLElement>();

	// The page scrolls inside its own pane, which clips the title as it goes
	// under the bar, so any clipping at all means it is on its way out.
	$effect(() => {
		if (!row || !oncollapse) return;
		const report = oncollapse;
		const observer = new IntersectionObserver(([entry]) => report(entry.intersectionRatio < 1), {
			threshold: 1
		});
		observer.observe(row);
		return () => {
			observer.disconnect();
			report(false);
		};
	});

	const heading = $derived(
		selected
			? new Date(`${selected}T00:00:00`).toLocaleDateString(
					getLocale(),
					compact
						? { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' }
						: { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }
				)
			: ''
	);

	// The page has its heading; the copy in the bar is not a second one.
	const tag = $derived(compact ? 'span' : 'h1');
	const title = $derived(
		compact
			? 'min-w-0 truncate text-sm font-semibold'
			: 'min-w-0 truncate text-2xl font-semibold tracking-tight'
	);
	const dayTitle = $derived(
		compact ? 'text-sm font-semibold whitespace-nowrap' : 'text-2xl font-semibold tracking-tight'
	);
</script>

<div
	bind:this={row}
	class={compact ? 'flex max-w-sm min-w-0 items-center gap-1' : 'mt-6 mb-8 flex items-center gap-2'}
>
	{#if timeline.kind !== 'day'}
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onback}
			aria-label={m.page_back()}
			title={m.page_back()}
			class="text-muted-foreground hover:text-foreground"
		>
			<ArrowLeftIcon />
		</Button>
	{/if}
	{#if timeline.kind === 'calendar'}
		<svelte:element this={tag} class={title}>{m.calendar_title()}</svelte:element>
	{:else if timeline.kind === 'tags'}
		<svelte:element this={tag} class={title}>
			{m.tags_title()}
			<span class="font-normal text-muted-foreground">
				{m.tags_count({ count: tagCount })}
			</span>
		</svelte:element>
	{:else if timeline.kind === 'similar'}
		<svelte:element this={tag} class={title}>{m.note_similar()}</svelte:element>
	{:else if timeline.kind === 'search'}
		{@const words = queryWords(timeline.query)}
		<svelte:element this={tag} class={title}>
			{m.page_results({ count: resultCount })}
			{#if words}<span class="font-normal text-muted-foreground">{words}</span>{/if}
		</svelte:element>
	{:else}
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={oncalendar}
			aria-label={m.calendar_pick()}
			title={m.calendar_pick()}
			class="text-muted-foreground hover:text-foreground"
		>
			<CalendarIcon />
		</Button>
		<svelte:element this={tag} class={dayTitle}>{heading}</svelte:element>
	{/if}
</div>
