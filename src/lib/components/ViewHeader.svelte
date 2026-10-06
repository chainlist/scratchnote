<script lang="ts" module>
	import { getLocale } from '#lib/paraglide/runtime.js';

	/** The formats `dayHeading` made, by language and length: making one
	 *  takes far longer than using it, and a thread heads every day with one. */
	const headings: Record<string, Intl.DateTimeFormat> = {};

	/** A day as its title reads, or shorter, as the top bar has it. */
	export function dayHeading(date: string, short = false) {
		const day = new Date(`${date}T00:00:00`);
		// A view being left reads its title once more with the next route's
		// data, which may hold no date: as toLocaleDateString did, no error.
		if (Number.isNaN(day.getTime())) return String(day);
		const format = (headings[`${getLocale()} ${short}`] ??= new Intl.DateTimeFormat(
			getLocale(),
			short
				? { weekday: 'short', day: 'numeric', month: 'short', year: 'numeric' }
				: { weekday: 'long', day: 'numeric', month: 'long', year: 'numeric' }
		));
		return format.format(day);
	}

	/** The formats `shortDay` made, by language and whether the year shows. */
	const shortDays: Record<string, Intl.DateTimeFormat> = {};

	/** A day in a few characters, as a line names it: the year only when it is not this one. */
	export function shortDay(date: string) {
		const day = new Date(`${date}T00:00:00`);
		const thisYear = day.getFullYear() === new Date().getFullYear();
		const format = (shortDays[`${getLocale()} ${thisYear}`] ??= new Intl.DateTimeFormat(
			getLocale(),
			{ day: 'numeric', month: 'short', ...(thisYear ? {} : { year: 'numeric' }) }
		));
		return format.format(day);
	}
</script>

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
		const mine: ViewTitle = { back, title, pin, detail, heading };
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
			class={compact
				? 'min-w-0 truncate text-sm font-semibold'
				: 'min-w-0 truncate text-2xl font-semibold tracking-tight'}
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
			aria-label={label}
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
