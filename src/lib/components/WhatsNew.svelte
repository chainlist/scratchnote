<script lang="ts">
	import type { Release } from '$lib/changelog';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Tabs from '$lib/components/ui/tabs';
	import Markdown from '$lib/components/Markdown.svelte';
	import BugIcon from '@lucide/svelte/icons/bug';
	import GaugeIcon from '@lucide/svelte/icons/gauge';
	import ListIcon from '@lucide/svelte/icons/list';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import TriangleAlertIcon from '@lucide/svelte/icons/triangle-alert';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';

	let {
		releases = $bindable()
	}: {
		/** Newest first, from CHANGELOG.md, in English whatever the language; null closes it. */
		releases: Release[] | null;
	} = $props();

	/**
	 * The sections release-please writes, in the order the sidebar lists them
	 * and named in the interface language. Any other keeps its English title.
	 */
	const KINDS: Record<string, { label: () => string; icon: typeof ListIcon }> = {
		'⚠ BREAKING CHANGES': { label: m.whats_new_breaking, icon: TriangleAlertIcon },
		Features: { label: m.whats_new_features, icon: SparklesIcon },
		'Bug Fixes': { label: m.whats_new_fixes, icon: BugIcon },
		'Performance Improvements': { label: m.whats_new_performance, icon: GaugeIcon }
	};
	const order = Object.keys(KINDS);
	const rank = (title: string) => (order.includes(title) ? order.indexOf(title) : order.length);

	/** Every kind of change in these releases, one sidebar entry each. */
	const titles = $derived(
		[...new Set((releases ?? []).flatMap((r) => r.sections.map((s) => s.title)))].sort(
			(a, b) => rank(a) - rank(b)
		)
	);

	const releaseDate = (date: string) =>
		new Date(`${date}T00:00:00`).toLocaleDateString(getLocale(), {
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		});
</script>

<Dialog.Root
	open={releases !== null}
	onOpenChange={(open) => {
		if (!open) releases = null;
	}}
>
	<Dialog.Content
		class="h-[min(640px,85vh)] grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-[min(56rem,calc(100%-2rem))]"
	>
		<Tabs.Root value={titles[0]} orientation="vertical" class="h-full min-h-0 gap-0">
			<aside class="flex w-48 shrink-0 flex-col gap-4 border-r bg-muted/40 p-3">
				<Dialog.Title class="px-2 pt-1">{m.whats_new_title()}</Dialog.Title>
				<Tabs.List class="w-full gap-0.5 bg-transparent p-0">
					{#each titles as title (title)}
						{@const Icon = KINDS[title]?.icon ?? ListIcon}
						<Tabs.Trigger value={title} class="h-8 w-full flex-none justify-start gap-2 px-2">
							<Icon />
							{KINDS[title]?.label() ?? title}
						</Tabs.Trigger>
					{/each}
				</Tabs.List>
			</aside>

			<div class="min-w-0 flex-1 overflow-y-auto p-6">
				{#each titles as title (title)}
					<Tabs.Content value={title} class="flex flex-col gap-6">
						{#each releases ?? [] as release (release.version)}
							{@const entries = release.sections.find((s) => s.title === title)?.entries}
							{#if entries}
								<section class="flex flex-col gap-2">
									<h3 class="flex items-baseline gap-2">
										<span class="text-base font-semibold">{release.version}</span>
										<span class="text-xs text-muted-foreground">{releaseDate(release.date)}</span>
									</h3>
									<Markdown text={entries.map((e) => `* ${e}`).join('\n')} class="text-sm" />
								</section>
							{/if}
						{/each}
					</Tabs.Content>
				{/each}
			</div>
		</Tabs.Root>
	</Dialog.Content>
</Dialog.Root>
