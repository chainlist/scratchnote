<script lang="ts">
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import { scale } from 'svelte/transition';

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

	/** The group of each initial, for the rail to scroll to. */
	const sections: HTMLElement[] = [];
	let index = $state<HTMLElement>();
	let rail = $state<HTMLElement>();
	/** The group the rail's thumb sits on. */
	let current = $state(0);
	let dragging = $state(false);

	// The rail follows the scroll, except while it is the one scrolling.
	$effect(() => {
		let scroller = index?.parentElement;
		while (scroller && !/auto|scroll/.test(getComputedStyle(scroller).overflowY)) {
			scroller = scroller.parentElement;
		}
		if (!scroller) return;
		const box = scroller;
		function onscroll() {
			if (dragging) return;
			// The last groups never reach the top, so the bottom of the page is the last letter.
			if (box.scrollTop + box.clientHeight >= box.scrollHeight - 2) {
				current = sections.length - 1;
				return;
			}
			const top = box.getBoundingClientRect().top + 24;
			current = Math.max(
				0,
				sections.findLastIndex((section) => section?.getBoundingClientRect().top <= top)
			);
		}
		box.addEventListener('scroll', onscroll, { passive: true });
		return () => box.removeEventListener('scroll', onscroll);
	});

	/** Put the thumb on the letter under the pointer and bring its group up. */
	function slide(y: number) {
		if (!rail) return;
		const box = rail.getBoundingClientRect();
		const at = Math.floor(((y - box.top) / box.height) * groups.length);
		const next = Math.min(groups.length - 1, Math.max(0, at));
		if (next === current && dragging) return;
		current = next;
		sections[next]?.scrollIntoView({ block: 'start' });
	}

	function onpointerdown(e: PointerEvent) {
		e.preventDefault();
		rail?.setPointerCapture(e.pointerId);
		dragging = true;
		slide(e.clientY);
	}

	function onpointermove(e: PointerEvent) {
		if (dragging) slide(e.clientY);
	}

	// Scroll events land before the next frame, so the jump the last move
	// caused cannot pull the thumb back onto a group that fits the page.
	function onpointerup() {
		requestAnimationFrame(() => (dragging = false));
	}

	// The rail is fixed to the window's edge, so a narrow window keeps the page clear of it.
	const inset = $derived(groups.length > 1 ? 'pr-6 lg:pr-0' : '');

	const label = 'mb-3 text-xs font-medium tracking-wider text-muted-foreground uppercase';
</script>

{#if tags.length === 0}
	<p class="text-base text-neutral-600">{m.tags_empty()}</p>
{:else}
	{#if categories.length > 0}
		<section class="mb-10 {inset}">
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
		<section bind:this={index} class={inset}>
			<h2 class={label}>{m.tags_all()}</h2>
			<div class="divide-y divide-border/60">
				{#each groups as group, i (group.initial)}
					<div bind:this={sections[i]} class="grid scroll-mt-4 grid-cols-[2rem_1fr] gap-x-3 py-2">
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

		{#if groups.length > 1}
			<!-- A pointer shortcut only: the tags above are all reachable by keyboard. -->
			<div class="fixed top-16 right-3 bottom-20 z-20 flex items-center" aria-hidden="true">
				<div
					bind:this={rail}
					role="presentation"
					{onpointerdown}
					{onpointermove}
					{onpointerup}
					onpointercancel={onpointerup}
					class="relative flex max-h-full w-6 cursor-pointer touch-none flex-col rounded-full py-1 transition-colors select-none {dragging
						? 'bg-muted/70'
						: 'hover:bg-muted/40'}"
				>
					<span
						class="absolute inset-x-0 rounded-full bg-primary/15 ring-1 ring-primary/30 transition-[top] duration-150 ease-out"
						style:top="calc(0.25rem + (100% - 0.5rem) * {current / groups.length})"
						style:height="calc((100% - 0.5rem) / {groups.length})"
					></span>
					{#if dragging}
						<!-- The letter under the finger, big enough to read past it. -->
						<div
							transition:scale={{ duration: 120, start: 0.6 }}
							class="pointer-events-none absolute right-full mr-4 -translate-y-1/2 transition-[top] duration-150 ease-out"
							style:top="calc(0.25rem + (100% - 0.5rem) * {(current + 0.5) / groups.length})"
						>
							<!-- A drop whose point faces the rail. -->
							<div
								class="flex size-16 -rotate-45 items-center justify-center rounded-[50%_50%_0_50%] bg-primary text-primary-foreground shadow-lg"
							>
								<span class="rotate-45 font-mono text-3xl font-semibold">
									{groups[current]?.initial}
								</span>
							</div>
						</div>
					{/if}
					{#each groups as group, i (group.initial)}
						<span
							class="relative flex h-5 min-h-0 shrink items-center justify-center font-mono text-[11px] leading-none transition-transform duration-150 ease-out {i ===
							current
								? 'scale-[1.6] font-semibold text-foreground'
								: 'text-muted-foreground'}">{group.initial}</span
						>
					{/each}
				</div>
			</div>
		{/if}
	{/if}
{/if}
