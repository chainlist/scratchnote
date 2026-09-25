<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';

	let {
		tags,
		categories,
		onpick
	}: {
		/** Every tag with its count, categories included. */
		tags: [string, number][];
		categories: [string, number][];
		/** List the notes carrying a tag. */
		onpick: (tag: string) => void;
	} = $props();

	// A category is also a tag, so it is left out of the index below it.
	const plain = $derived.by(() => {
		const names = new Set(categories.map(([name]) => name));
		return tags.filter(([name]) => !names.has(name));
	});

	const topCategory = $derived(Math.max(1, ...categories.map(([, count]) => count)));
	const topTag = $derived(Math.max(1, ...plain.map(([, count]) => count)));

	/** Tags under their initial, in the interface language's order. Digits and symbols go under `#`. */
	const groups = $derived.by(() => {
		const locale = getLocale();
		const collator = new Intl.Collator(locale, { sensitivity: 'base', numeric: true });
		const sorted = [...plain].sort((a, b) => collator.compare(a[0], b[0]));
		const out: { initial: string; tags: [string, number][] }[] = [];
		for (const entry of sorted) {
			const first = entry[0].normalize('NFD').charAt(0).toLocaleUpperCase(locale);
			const initial = /\p{L}/u.test(first) ? first : '#';
			const last = out.at(-1);
			if (last?.initial === initial) last.tags.push(entry);
			else out.push({ initial, tags: [entry] });
		}
		return out;
	});

	/** How strongly a tag reads, on a log scale so one busy tag does not wash out the rest. */
	function tone(count: number) {
		const weight = topTag > 1 ? Math.log(count) / Math.log(topTag) : 1;
		if (weight > 0.66) return 'text-foreground font-medium';
		if (weight > 0.33) return 'text-foreground/80';
		return 'text-muted-foreground';
	}

	const label = 'mb-3 text-xs font-medium tracking-wider text-muted-foreground uppercase';
</script>

{#if tags.length === 0}
	<p class="text-base text-neutral-600">{m.tags_empty()}</p>
{:else}
	{#if categories.length > 0}
		<section class="mb-10">
			<h2 class={label}>{m.categories_heading()}</h2>
			<div class="grid grid-cols-2 gap-2 sm:grid-cols-3">
				{#each categories as [name, count] (name)}
					<button
						type="button"
						onclick={() => onpick(name)}
						title={m.note_show_tag({ tag: name })}
						class="flex cursor-pointer flex-col gap-3 rounded-lg border bg-card/40 p-3 text-left transition-colors hover:border-input hover:bg-muted/50"
					>
						<span class="flex items-baseline justify-between gap-2">
							<span class="truncate font-mono text-sm">#{name}</span>
							<span class="shrink-0 text-xs text-muted-foreground">
								{m.tags_notes({ count })}
							</span>
						</span>
						<span class="h-1 overflow-hidden rounded-full bg-muted" aria-hidden="true">
							<span
								class="block h-full rounded-full bg-primary/70"
								style:width="{(count / topCategory) * 100}%"
							></span>
						</span>
					</button>
				{/each}
			</div>
		</section>
	{/if}

	{#if plain.length > 0}
		<section>
			<h2 class={label}>{m.tags_all()}</h2>
			<div class="divide-y divide-border/60">
				{#each groups as group (group.initial)}
					<div class="grid grid-cols-[2rem_1fr] gap-x-3 py-2">
						<span
							class="sticky top-2 self-start pt-1 font-mono text-sm text-muted-foreground/70"
							aria-hidden="true">{group.initial}</span
						>
						<div class="flex flex-wrap gap-1">
							{#each group.tags as [name, count] (name)}
								<button
									type="button"
									onclick={() => onpick(name)}
									title={m.note_show_tag({ tag: name })}
									class="inline-flex h-7 cursor-pointer items-center gap-1.5 rounded-md border border-transparent px-2 font-mono text-sm transition-colors hover:border-input hover:bg-input/30 {tone(
										count
									)}"
								>
									#{name}
									<span class="text-xs font-normal text-muted-foreground/70">{count}</span>
								</button>
							{/each}
						</div>
					</div>
				{/each}
			</div>
		</section>
	{/if}
{/if}
