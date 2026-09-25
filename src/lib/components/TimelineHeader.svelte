<script lang="ts">
	import type { DaySummary } from '$lib/api';
	import DayCalendar from '$lib/components/DayCalendar.svelte';
	import { Button } from '$lib/components/ui/button';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import { queryWords } from '$lib/query';
	import type { Timeline } from '$lib/timeline';

	let {
		timeline,
		days,
		selected,
		tagCount,
		resultCount,
		onback,
		onselect
	}: {
		timeline: Timeline;
		days: DaySummary[];
		/** The day shown, and the one to go back to. */
		selected: string;
		tagCount: number;
		resultCount: number;
		onback: () => void;
		onselect: (date: string) => void;
	} = $props();

	const heading = $derived(
		selected
			? new Date(`${selected}T00:00:00`).toLocaleDateString(getLocale(), {
					weekday: 'long',
					day: 'numeric',
					month: 'long',
					year: 'numeric'
				})
			: ''
	);

	const title = 'min-w-0 truncate text-2xl font-semibold tracking-tight';
</script>

<div class="mt-6 mb-8 flex items-center gap-2">
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
	{#if timeline.kind === 'tags'}
		<h1 class={title}>
			{m.tags_title()}
			<span class="font-normal text-muted-foreground">
				{m.tags_count({ count: tagCount })}
			</span>
		</h1>
	{:else if timeline.kind === 'similar'}
		<h1 class={title}>{m.note_similar()}</h1>
	{:else if timeline.kind === 'search'}
		{@const words = queryWords(timeline.query)}
		<h1 class={title}>
			{m.page_results({ count: resultCount })}
			{#if words}<span class="font-normal text-muted-foreground">{words}</span>{/if}
		</h1>
	{:else}
		<DayCalendar {days} {selected} {onselect} />
		<h1 class="text-2xl font-semibold tracking-tight">{heading}</h1>
	{/if}
</div>
