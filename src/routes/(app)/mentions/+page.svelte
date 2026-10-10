<script lang="ts">
	import type { MentionSummary, Pin } from '#lib/api.js';
	import View from '#lib/components/layout/View.svelte';
	import MentionLetter from '#lib/components/common/MentionLetter.svelte';
	import PinButton from '#lib/components/common/PinButton.svelte';
	import ShowMore from '#lib/components/common/ShowMore.svelte';
	import { daysAgo, shortDay } from '#lib/dates.js';
	import RouteIcon from '@lucide/svelte/icons/route';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { goto } from '$app/navigation';
	import * as InputGroup from '#lib/components/ui/input-group/index.js';
	import { hasEvery, queryWords } from '#lib/matching.js';
	import { along, mentionHref, mentionHue } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();

	/** The days a name's pulse shows, twelve weeks, and the days after which
	 *  it has gone quiet. */
	const SPAN = 84;
	const QUIET = 30;
	/** How many names come up as in hand besides the pinned. */
	const IN_HAND = 6;
	/** How many of the others are drawn at a time. */
	const PAGE = 30;

	/** A name gone quiet reads in the quiet tone, its facts still readable
	 *  under it; its letter and pulse fade. All come back under the pointer. */
	const nameTone = (quiet: boolean) =>
		quiet
			? 'text-meta transition-colors group-hover:text-neutral-100 group-has-focus-visible:text-neutral-100'
			: 'text-neutral-100';

	const pinOf = (mention: MentionSummary): Pin => ({
		kind: 'mention',
		target: mention.key,
		label: mention.name
	});

	/** On how many days a name came up since it would have gone quiet. */
	const lately = (mention: MentionSummary) =>
		mention.days.filter((day) => daysAgo(day) <= QUIET).length;

	/** The names in hand: the pinned, then those that came up on the most days
	 *  lately, the last named first among equals. */
	const inHand = $derived.by(() => {
		const pinned = data.mentions.filter((mention) => shell.isPinned(pinOf(mention)));
		const active = data.mentions
			.filter((mention) => !shell.isPinned(pinOf(mention)) && lately(mention) > 0)
			.toSorted((a, b) => lately(b) - lately(a) || b.last.localeCompare(a.last))
			.slice(0, IN_HAND);
		return [...pinned, ...active];
	});

	/** The rest, as they come: most mentioned first. */
	const others = $derived.by(() => {
		const keys = new Set(inHand.map((mention) => mention.key));
		return data.mentions.filter((mention) => !keys.has(mention.key));
	});

	/** What is typed to find a name. */
	let query = $state('');

	/** The names of `mentions` holding every word typed, in the same order. */
	const found = (mentions: MentionSummary[]) => {
		const words = queryWords(query);
		return mentions.filter((mention) => hasEvery(mention.name, words));
	};
	const shownInHand = $derived(found(inHand));
	const shownOthers = $derived(found(others));

	/** How many of the others are drawn, a page more at a time: a space can
	 *  mention hundreds of names. Back to one page as what is typed changes. */
	let drawn = $derived.by(() => {
		void query;
		return PAGE;
	});

	/** How many threads of the user's each name holds, by key (SPEC 6.4). */
	const threads = $derived.by(() => {
		const counts: Record<string, number> = {};
		for (const thread of shell.threads.threadList)
			if (thread.scope && thread.kept) counts[thread.scope] = (counts[thread.scope] ?? 0) + 1;
		return counts;
	});
</script>

<!-- A name's letter in its own colour. -->
{#snippet letter(mention: MentionSummary, quiet: boolean)}
	<MentionLetter
		name={mention.name}
		class={[
			quiet &&
				'opacity-50 transition-opacity group-hover:opacity-100 group-has-focus-visible:opacity-100'
		]}
	/>
{/snippet}

<!-- How many notes name it, the day of the last, and how many threads it holds. -->
{#snippet facts(mention: MentionSummary, quiet: boolean)}
	<span class="flex items-center gap-3 text-xs text-meta">
		<span class="truncate">
			{(quiet ? m.mentions_quiet : m.thread_last)({
				count: mention.notes,
				date: shortDay(mention.last)
			})}
		</span>
		{#if shell.canSimilar && threads[mention.key]}
			<span class="flex shrink-0 items-center gap-1" title={m.threads_all()}>
				<RouteIcon class="size-3" />{threads[mention.key]}
			</span>
		{/if}
	</span>
{/snippet}

<!-- A dot a day it came up on a line of the last twelve weeks, ticked every
     four, today at the right end. -->
{#snippet pulse(at: number[], wide: boolean, quiet: boolean)}
	<span
		class={[
			'relative h-3',
			wide ? 'w-full' : 'w-24 shrink-0',
			quiet &&
				'opacity-50 transition-opacity group-hover:opacity-100 group-has-focus-visible:opacity-100'
		]}
		title={m.mentions_pulse()}
		aria-hidden="true"
	>
		{#each [1 / 3, 2 / 3] as tick (tick)}
			<span class="absolute inset-y-0 w-px bg-neutral-800/60" style:left="{tick * 100}%"></span>
		{/each}
		<span class="absolute inset-x-0 top-1/2 h-px -translate-y-1/2 bg-neutral-800"></span>
		{#each at as day, i (i)}
			<span
				class={[
					'name-mark absolute top-1/2 -translate-y-1/2 rounded-full',
					wide ? 'size-2' : 'size-1.5'
				]}
				style:left="calc({day} * (100% - {wide ? 0.5 : 0.375}rem))"
			></span>
		{/each}
	</span>
{/snippet}

<!-- Its pin to the left edge, shown on hover unless pinned. -->
{#snippet pinButton(mention: MentionSummary, extra: string)}
	<PinButton pin={pinOf(mention)} hoverOnly class={extra} />
{/snippet}

<!-- Every name mentioned. Those in hand, the pinned and those that came up
     most lately, are cards on top, each with its pulse over the last twelve
     weeks. The others are a list below, the most mentioned first, a pulse
     only for those that came up on two days or more of them, a page at a
     time with Show more and how many are left under it. Each shows its
     letter in its own colour, how many notes name it, the day of the last,
     how many threads it holds and a pin to the left edge. A name gone quiet
     fades, but for its facts. A box above finds a name: Enter opens the first found, Esc
     clears it (SPEC 3.10). -->
<View back={shell.back} title={m.mentions_title()}>
	{#if data.mentions.length === 0}
		<p class="text-base text-meta">{m.mentions_none()}</p>
	{:else}
		{@const headed = shownInHand.length > 0 && shownOthers.length > 0}
		<InputGroup.Root class="mb-6 h-8">
			<InputGroup.Addon><SearchIcon /></InputGroup.Addon>
			<InputGroup.Input
				bind:value={query}
				placeholder={m.mentions_search()}
				aria-label={m.mentions_search()}
				onkeydown={(event) => {
					const first = shownInHand[0] ?? shownOthers[0];
					if (event.key === 'Escape' && query) {
						event.preventDefault();
						query = '';
					} else if (event.key === 'Enter' && query && first) {
						event.preventDefault();
						void goto(mentionHref(first.name));
					}
				}}
			/>
		</InputGroup.Root>

		{#if shownInHand.length > 0}
			<section class="@container mb-8">
				{#if headed}
					<h2 class="mb-2 text-xs font-medium text-meta">{m.mentions_active()}</h2>
				{/if}
				<ul class="grid grid-cols-1 gap-2 @md:grid-cols-2 @2xl:grid-cols-3">
					{#each shownInHand as mention (mention.key)}
						{@const quiet = daysAgo(mention.last) > QUIET}
						<li class="group relative" style:--hue={mentionHue(mention.key)}>
							<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
							<a
								href={mentionHref(mention.name)}
								class="flex h-full flex-col gap-4 rounded-lg border border-neutral-800 p-3 transition-colors hover:border-neutral-700 hover:bg-neutral-900 has-focus-visible:border-neutral-700 has-focus-visible:bg-neutral-900"
							>
								<span class="flex min-w-0 items-center gap-2.5 pr-7">
									{@render letter(mention, quiet)}
									<span class="min-w-0 flex-1">
										<span class={['block truncate font-medium', nameTone(quiet)]}
											>@{mention.name}</span
										>
										{@render facts(mention, quiet)}
									</span>
								</span>
								{@render pulse(along(mention.days, SPAN), true, quiet)}
							</a>
							{@render pinButton(mention, 'absolute top-2 right-2')}
						</li>
					{/each}
				</ul>
			</section>
		{/if}

		{#if shownOthers.length > 0}
			<section>
				{#if headed}
					<h2 class="mb-1 text-xs font-medium text-meta">{m.mentions_others()}</h2>
				{/if}
				<ul>
					{#each shownOthers.slice(0, drawn) as mention (mention.key)}
						{@const quiet = daysAgo(mention.last) > QUIET}
						{@const at = along(mention.days, SPAN)}
						<li
							class="group -mx-2 flex items-center gap-1 rounded hover:bg-neutral-900 has-focus-visible:bg-neutral-900"
							style:--hue={mentionHue(mention.key)}
						>
							<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
							<a
								href={mentionHref(mention.name)}
								class="flex min-w-0 flex-1 items-center gap-3 py-2 pl-2"
							>
								{@render letter(mention, quiet)}
								<span class="min-w-0 flex-1">
									<span class={['block truncate', nameTone(quiet)]}>@{mention.name}</span>
									{@render facts(mention, quiet)}
								</span>
								{#if at.length >= 2}
									{@render pulse(at, false, quiet)}
								{/if}
							</a>
							{@render pinButton(mention, 'mr-1')}
						</li>
					{/each}
				</ul>
				{#if drawn < shownOthers.length}
					<ShowMore
						class="mt-4"
						count={shownOthers.length - drawn}
						onclick={() => (drawn += PAGE)}
					/>
				{/if}
			</section>
		{/if}

		{#if shownInHand.length === 0 && shownOthers.length === 0}
			<p class="text-sm text-meta">{m.mentions_no_match()}</p>
		{/if}
	{/if}
</View>
