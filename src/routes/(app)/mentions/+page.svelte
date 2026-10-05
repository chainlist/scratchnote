<script lang="ts">
	import type { MentionSummary, Pin } from '#lib/api.js';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import PinIcon from '@lucide/svelte/icons/pin';
	import RouteIcon from '@lucide/svelte/icons/route';
	import { mentionHref } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();

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

	/** How many threads of the user's each name holds, by key (SPEC 6.4). */
	const threads = $derived.by(() => {
		const counts: Record<string, number> = {};
		for (const thread of shell.threadList)
			if (thread.scope && thread.kept) counts[thread.scope] = (counts[thread.scope] ?? 0) + 1;
		return counts;
	});
</script>

<!-- Every name mentioned, the pinned first, then the most mentioned, each
     with how many notes name it, the day of the last and how many threads
     it holds, and a pin to the left edge (SPEC 3.10). -->
<View back={shell.back} title={m.mentions_title()}>
	{#if mentions.length === 0}
		<p class="text-base text-neutral-600">{m.mentions_none()}</p>
	{:else}
		<ul>
			{#each mentions as mention (mention.key)}
				{@const pin = pinOf(mention)}
				{@const pinned = shell.isPinned(pin)}
				{@const label = pinned ? m.pin_remove() : m.pin_add()}
				<li class="group -mx-2 flex items-center gap-1 rounded hover:bg-neutral-900">
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
					<a
						href={mentionHref(mention.name)}
						class="flex min-w-0 flex-1 items-baseline justify-between gap-4 py-2 pl-2"
					>
						<span class="mention truncate">@{mention.name}</span>
						<span class="flex shrink-0 items-center gap-3 text-xs text-neutral-500">
							{#if shell.canSimilar && threads[mention.key]}
								<span class="flex items-center gap-1" title={m.threads_all()}>
									<RouteIcon class="size-3" />{threads[mention.key]}
								</span>
							{/if}
							{m.thread_last({ count: mention.notes, date: shortDay(mention.last) })}
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
	{/if}
</View>
