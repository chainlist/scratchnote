<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import ArrowLeftIcon from '@lucide/svelte/icons/arrow-left';
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import ChevronLeftIcon from '@lucide/svelte/icons/chevron-left';
	import ChevronRightIcon from '@lucide/svelte/icons/chevron-right';
	import FilesIcon from '@lucide/svelte/icons/files';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import type { Timeline } from '$lib/timeline';

	let {
		timeline,
		selected,
		resultCount,
		onback,
		oncalendar,
		onpages,
		onprevious,
		onnext,
		oncollapse,
		compact = false
	}: {
		timeline: Timeline;
		/** The day shown, and the one to go back to. */
		selected: string;
		resultCount: number;
		onback: () => void;
		oncalendar: () => void;
		/** List every page of the space. */
		onpages: () => void;
		/** Step to the neighbouring day; left out when there is none that way. */
		onprevious?: () => void;
		onnext?: () => void;
		/** Told when the title starts under the top bar, which then shows it instead, and when it is back. */
		oncollapse?: (collapsed: boolean) => void;
		/** The small copy the top bar shows in place of the scrolled-away title. */
		compact?: boolean;
	} = $props();

	const mac = navigator.userAgent.includes('Mac');

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
	{:else if timeline.kind === 'pages'}
		<svelte:element this={tag} class={title}>{m.pages_all()}</svelte:element>
	{:else if timeline.kind === 'similar'}
		<svelte:element this={tag} class={title}>{m.note_similar()}</svelte:element>
	{:else if timeline.kind === 'page'}
		<!-- The page's title is its own heading, in the view; this is its day. -->
		<span class="text-sm font-medium whitespace-nowrap text-muted-foreground">{heading}</span>
	{:else if timeline.kind === 'search'}
		<!-- The whole query, so a `#category` filter shows too. -->
		{@const shown = timeline.query.trim()}
		<svelte:element this={tag} class={title}>
			{m.page_results({ count: resultCount })}
			{#if shown}<span class="font-normal text-muted-foreground">{shown}</span>{/if}
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
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onpages}
			aria-label={m.pages_all()}
			title={m.pages_all()}
			class="text-muted-foreground hover:text-foreground"
		>
			<FilesIcon />
		</Button>
		<!-- Side by side ahead of the title, so they stay put while its width
		     changes from one day to the next. -->
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onprevious}
			disabled={!onprevious}
			aria-label={m.calendar_previous_day()}
			aria-keyshortcuts="Alt+ArrowLeft"
			title="{m.calendar_previous_day()} ({mac ? '⌥←' : 'Alt+←'})"
			class="text-muted-foreground hover:text-foreground"
		>
			<ChevronLeftIcon />
		</Button>
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onnext}
			disabled={!onnext}
			aria-label={m.calendar_next_day()}
			aria-keyshortcuts="Alt+ArrowRight"
			title="{m.calendar_next_day()} ({mac ? '⌥→' : 'Alt+→'})"
			class="text-muted-foreground hover:text-foreground"
		>
			<ChevronRightIcon />
		</Button>
		<svelte:element this={tag} class={dayTitle}>{heading}</svelte:element>
	{/if}
</div>
