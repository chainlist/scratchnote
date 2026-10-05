<script lang="ts">
	import type { MentionSummary, Pin } from '#lib/api.js';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { daysAgo } from '#lib/days.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import PinIcon from '@lucide/svelte/icons/pin';
	import RouteIcon from '@lucide/svelte/icons/route';
	import SearchIcon from '@lucide/svelte/icons/search';
	import { goto } from '$app/navigation';
	import * as InputGroup from '#lib/components/ui/input-group/index.js';
	import { hasEvery, queryWords } from '#lib/matching.js';
	import { mentionHref, mentionHue, weekly } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();

	/** The weeks a name's pulse shows, and the days after which it has gone quiet. */
	const WEEKS = 12;
	const QUIET = 30;

	const pinOf = (mention: MentionSummary): Pin => ({
		kind: 'mention',
		target: mention.key,
		label: mention.name
	});

	/** The pinned first, the projects in hand, then as they come: most mentioned first. */
	const mentions = $derived(
		data.mentions.toSorted(
			(a, b) => Number(shell.isPinned(pinOf(b))) - Number(shell.isPinned(pinOf(a)))
		)
	);

	/** What is typed to find a name. */
	let query = $state('');

	/** The names holding every word typed, in the same order. */
	const shown = $derived.by(() => {
		const words = queryWords(query);
		return mentions.filter((mention) => hasEvery(mention.name, words));
	});

	/** How many threads of the user's each name holds, by key (SPEC 6.4). */
	const threads = $derived.by(() => {
		const counts: Record<string, number> = {};
		for (const thread of shell.threadList)
			if (thread.scope && thread.kept) counts[thread.scope] = (counts[thread.scope] ?? 0) + 1;
		return counts;
	});
</script>

<!-- Every name mentioned, the pinned first, then the most mentioned, each
     under its letter in its own colour, with how many notes name it, the
     day of the last and how many threads it holds, its pulse over the last
     weeks, and a pin to the left edge. A name gone quiet fades. A box
     above finds a name: Enter opens the first found, Esc clears it (SPEC 3.10). -->
<View back={shell.back} title={m.mentions_title()}>
	{#if mentions.length === 0}
		<p class="text-base text-neutral-600">{m.mentions_none()}</p>
	{:else}
		<InputGroup.Root class="mb-4 h-8">
			<InputGroup.Addon><SearchIcon /></InputGroup.Addon>
			<InputGroup.Input
				bind:value={query}
				placeholder={m.mentions_search()}
				aria-label={m.mentions_search()}
				onkeydown={(event) => {
					if (event.key === 'Escape' && query) {
						event.preventDefault();
						query = '';
					} else if (event.key === 'Enter' && query && shown[0]) {
						event.preventDefault();
						void goto(mentionHref(shown[0].name));
					}
				}}
			/>
		</InputGroup.Root>
		<ul>
			{#each shown as mention (mention.key)}
				{@const pin = pinOf(mention)}
				{@const pinned = shell.isPinned(pin)}
				{@const label = pinned ? m.pin_remove() : m.pin_add()}
				{@const quiet = daysAgo(mention.last) > QUIET}
				<li
					class="group -mx-2 flex items-center gap-1 rounded hover:bg-neutral-900"
					style:--hue={mentionHue(mention.key)}
				>
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
					<a
						href={mentionHref(mention.name)}
						class={[
							'flex min-w-0 flex-1 items-center gap-3 py-2 pl-2 transition-opacity',
							quiet && 'opacity-50 group-hover:opacity-100'
						]}
					>
						<span
							class="name-tint flex size-8 shrink-0 items-center justify-center rounded-lg text-sm font-semibold"
						>
							{[...mention.name][0]?.toUpperCase()}
						</span>
						<span class="min-w-0 flex-1">
							<span class="block truncate text-neutral-100">@{mention.name}</span>
							<span class="flex items-center gap-3 text-xs text-neutral-500">
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
						</span>
						<!-- A bar a week, full for a name that came up every day of it; on a
						     square root, so a day or two a week still stands up. -->
						<span
							class="flex h-6 w-24 shrink-0 items-end gap-0.5"
							title={m.mentions_pulse()}
							aria-hidden="true"
						>
							{#each weekly(mention.days, WEEKS) as days, week (week)}
								<span
									class={['flex-1 rounded-[1px]', days ? 'name-mark' : 'bg-neutral-800']}
									style:height="{Math.max(2 / 24, Math.sqrt(days / 7)) * 100}%"
								></span>
							{/each}
						</span>
					</a>
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={() => void shell.togglePin(pin)}
						aria-label={label}
						aria-pressed={pinned}
						title={label}
						class={[
							'mr-1 shrink-0',
							pinned
								? 'text-foreground'
								: 'text-muted-foreground opacity-0 group-hover:opacity-100 hover:text-foreground focus-visible:opacity-100'
						]}
					>
						<PinIcon class={pinned ? 'fill-current' : undefined} />
					</Button>
				</li>
			{/each}
		</ul>
		{#if shown.length === 0}
			<p class="text-sm text-neutral-500">{m.mentions_no_match()}</p>
		{/if}
	{/if}
</View>
