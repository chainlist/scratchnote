<script lang="ts" generics="T extends Thread">
	import type { Snippet } from 'svelte';
	import type { Thread } from '#lib/api.js';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { daysAgo } from '#lib/days.js';
	import { mentionHue } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { threadHref } from '#lib/threads.js';

	let {
		threads,
		name = (thread) => thread.title ?? m.thread_untitled(),
		chips = true,
		actions
	}: {
		threads: T[];
		/** What a thread goes by: its title, else a generic one. */
		name?: (thread: T) => string;
		/** The name a thread was found among, before its title: left out
		 *  where every thread is of that one name. */
		chips?: boolean;
		/** What follows a thread's line, such as Keep thread and Dismiss. */
		actions?: Snippet<[T]>;
	} = $props();

	/** The days the lanes span back from today: back to the oldest thread,
	 *  but no less than a month, so a young space's dots do not sprawl, and
	 *  no more than half a year, so the last weeks keep room. */
	const span = $derived(
		Math.min(180, Math.max(30, ...threads.map((thread) => daysAgo(thread.since) + 1)))
	);

	/** Where a day falls along a lane, in percent from its left, today at its right end. */
	const at = (date: string) => (1 - daysAgo(date) / span) * 100;

	/** The first of each month within the span, to mark on the lanes. One
	 *  too near today is left out, as its name would run off the end. */
	const months = $derived.by(() => {
		const format = new Intl.DateTimeFormat(getLocale(), { month: 'short' });
		const marks: { left: number; label: string }[] = [];
		const now = new Date();
		for (let back = 0; ; back++) {
			const day = new Date(now.getFullYear(), now.getMonth() - back, 1);
			const month = String(day.getMonth() + 1).padStart(2, '0');
			const left = at(`${day.getFullYear()}-${month}-01`);
			if (left < 0) return marks;
			if (left < 92) marks.push({ left, label: format.format(day) });
		}
	});
</script>

<!-- Threads as lanes across the same stretch of days, a dot for each day a
     thread holds a note, today at the right: how long each ran and how
     often it came up, side by side (SPEC 6.4). A name's thread is in that
     name's hue, one only suggested fainter. -->
<div class="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-4 pb-1">
	<span></span>
	<span class="relative h-4 text-xs text-neutral-600" aria-hidden="true">
		{#each months as month (month.left)}
			<span class="absolute" style:left="{month.left}%">{month.label}</span>
		{/each}
	</span>
</div>
<ul>
	{#each threads as thread (thread.id)}
		{@const mark = thread.scope !== null ? 'name-mark' : 'bg-neutral-400'}
		<li
			class="-mx-2 flex items-center gap-1 rounded hover:bg-neutral-900"
			style:--hue={thread.scope && mentionHue(thread.scope)}
		>
			<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- threadHref resolves it -->
			<a
				href={threadHref(thread.id)}
				class="grid min-w-0 flex-1 grid-cols-[minmax(0,2fr)_minmax(0,3fr)] items-center gap-4 px-2 py-2"
			>
				<span class="min-w-0">
					<span class="block truncate text-neutral-200">
						{#if chips && thread.mention}<span
								class="name-tint mr-1.5 rounded-md px-1 text-sm font-medium">@{thread.mention}</span
							>{/if}{name(thread)}
					</span>
					<span class="block truncate text-xs text-neutral-500">
						{m.thread_last({ count: thread.notes.length, date: shortDay(thread.until) })}
						{#if !thread.kept}· {m.threads_suggested()}{/if}
					</span>
				</span>
				<span class={['relative h-4', !thread.kept && 'opacity-50']} aria-hidden="true">
					{#each months as month (month.left)}
						<span class="absolute inset-y-0 w-px bg-neutral-800/60" style:left="{month.left}%"
						></span>
					{/each}
					<span
						class={['absolute top-1/2 h-0.5 -translate-y-1/2 rounded-full opacity-40', mark]}
						style:left="{Math.max(0, at(thread.since))}%"
						style:right="{100 - Math.max(0, at(thread.until))}%"
					></span>
					{#each thread.days as day (day)}
						{#if at(day) >= 0}
							<span
								class={['absolute top-1/2 size-2 -translate-1/2 rounded-full', mark]}
								style:left="{at(day)}%"
							></span>
						{/if}
					{/each}
				</span>
			</a>
			{@render actions?.(thread)}
		</li>
	{/each}
</ul>
