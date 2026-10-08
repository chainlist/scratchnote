<script lang="ts" generics="T extends Thread">
	import type { Snippet } from 'svelte';
	import type { Thread } from '#lib/api.js';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { daysAgo } from '#lib/days.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { threadHref, threadHue } from '#lib/threads.js';

	const shell = getShell();

	let {
		threads,
		name = (thread) => shell.nameOf(thread),
		chips = true,
		actions
	}: {
		threads: T[];
		/** What a thread goes by: by default, its name as everywhere (`nameOf`). */
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

	/** How far along a lane a day falls, from 0 at its left to 1 for today. */
	const along = (date: string) => 1 - daysAgo(date) / span;

	/** Where along a lane falls, kept a dot's half from either end so the
	 *  first and last dots are whole. */
	const x = (at: number) => `calc(${Math.max(0, at)} * (100% - 0.5rem) + 0.25rem)`;

	/** The first of each month within the span, each a line down the lanes,
	 *  named just after it. One too near the first day shown is left
	 *  unnamed, as is one too near the right end for its name to fit. */
	const months = $derived.by(() => {
		const format = new Intl.DateTimeFormat(getLocale(), { month: 'short' });
		const marks: { at: number; label: string | null }[] = [];
		const now = new Date();
		for (let back = 0; ; back++) {
			const day = new Date(now.getFullYear(), now.getMonth() - back, 1);
			const month = String(day.getMonth() + 1).padStart(2, '0');
			const at = along(`${day.getFullYear()}-${month}-01`);
			if (at < 0) return marks;
			marks.push({ at, label: at > 0.15 && at < 0.9 ? format.format(day) : null });
		}
	});

	/** The first day the lanes show. */
	const first = $derived.by(() => {
		const now = new Date();
		const day = new Date(now.getFullYear(), now.getMonth(), now.getDate() - span);
		const pad = (n: number) => String(n).padStart(2, '0');
		return `${day.getFullYear()}-${pad(day.getMonth() + 1)}-${pad(day.getDate())}`;
	});

	const range = (thread: T) =>
		thread.since === thread.until
			? shortDay(thread.since)
			: m.thread_range({ since: shortDay(thread.since), until: shortDay(thread.until) });
</script>

<!-- A line down the lanes at the first of each month. -->
{#snippet gridlines()}
	{#each months as month (month.at)}
		<span class="absolute inset-y-0 w-px bg-neutral-800" style:left={x(month.at)}></span>
	{/each}
{/snippet}

<!-- Threads as lanes across the same stretch of days in one framed list, a
     dot for each day a thread holds a note, today at the right: how long
     each ran and how often it came up, side by side (SPEC 6.4). Above them,
     the first day shown and each month named at its line, so a name reads
     as the line's rather than a column's. A name's thread is in that
     name's hue, one of no name in the accent, one only suggested fainter.
     A whole row opens its thread. -->
<div class="overflow-hidden rounded-lg border border-neutral-800">
	<div
		class="grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-4 border-b border-neutral-800 bg-neutral-900/50 px-3"
		aria-hidden="true"
	>
		<span></span>
		<span class="relative h-7 text-xs text-meta">
			{@render gridlines()}
			<span class="absolute top-1/2 left-1 -translate-y-1/2">{shortDay(first)}</span>
			{#each months as month (month.at)}
				{#if month.label}
					<span class="absolute top-1/2 -translate-y-1/2 pl-1.5" style:left={x(month.at)}>
						{month.label}
					</span>
				{/if}
			{/each}
		</span>
	</div>
	<ul class="divide-y divide-neutral-800">
		{#each threads as thread (thread.id)}
			<li
				class="relative grid grid-cols-[minmax(0,2fr)_minmax(0,3fr)] gap-4 px-3 transition-colors hover:bg-neutral-900 has-focus-visible:bg-neutral-900"
				style:--hue={threadHue(thread.id)}
			>
				<!-- The actions go under the name when the two do not fit, so the
				     name never shrinks to nothing. -->
				<div class="flex min-w-0 flex-wrap items-center gap-x-2 gap-y-1.5 py-2.5">
					<div class="min-w-0 flex-1 basis-32">
						<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- threadHref resolves it -->
						<a
							href={threadHref(thread.id)}
							title={range(thread)}
							class="block truncate text-neutral-200 after:absolute after:inset-0"
						>
							{#if chips && thread.mention}<span
									class="name-tint mr-1.5 rounded-md px-1 text-sm font-medium"
									>@{thread.mention}</span
								>{/if}{name(thread)}
						</a>
						<span class="block truncate text-xs text-meta">
							{m.thread_last({ count: thread.notes.length, date: shortDay(thread.until) })}
							{#if !thread.kept}· {m.threads_suggested()}{/if}
						</span>
					</div>
					{#if actions}
						<div class="relative flex shrink-0 items-center gap-1">{@render actions(thread)}</div>
					{/if}
				</div>
				<span
					class={['pointer-events-none relative', !thread.kept && 'opacity-50']}
					aria-hidden="true"
				>
					{@render gridlines()}
					<span class="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-neutral-800"></span>
					<span
						class="name-mark absolute top-1/2 h-0.5 -translate-y-1/2 rounded-full opacity-40"
						style:left={x(along(thread.since))}
						style:width="calc({along(thread.until) - Math.max(0, along(thread.since))} * (100% - 0.5rem))"
					></span>
					{#each thread.days as day (day)}
						{#if along(day) >= 0}
							<span
								class="name-mark absolute top-1/2 size-2 -translate-1/2 rounded-full"
								style:left={x(along(day))}
							></span>
						{/if}
					{/each}
				</span>
			</li>
		{/each}
	</ul>
</div>
