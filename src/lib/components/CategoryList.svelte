<script lang="ts">
	import { sidebarItem } from '$lib/components/sidebar';
	import { m } from '$lib/paraglide/messages';

	let {
		categories,
		active,
		onselect
	}: {
		categories: [string, number][];
		/** Tags currently filtered on; a category among them is highlighted. */
		active: string[];
		/** `additive` is true when Ctrl (Cmd on macOS) is held. */
		onselect: (category: string, additive: boolean) => void;
	} = $props();

	/** Rows shown before "More", so the calendar keeps its room. */
	const TOP = 8;

	let expanded = $state(false);

	// An active category past the fold is still shown, so the highlight is
	// never hidden behind "More".
	const shown = $derived(
		expanded ? categories : categories.filter(([name], i) => i < TOP || active.includes(name))
	);
</script>

{#if categories.length > 0}
	<!-- Fills what the sidebar has left; only the list scrolls, the heading stays. -->
	<nav aria-label={m.categories_heading()} class="flex min-h-0 flex-1 flex-col">
		<h2
			class="mb-1 px-2 text-[0.6875rem] font-medium tracking-wide text-muted-foreground uppercase"
		>
			{m.categories_heading()}
		</h2>
		<ul class="flex min-h-0 flex-col gap-0.5 overflow-y-auto">
			{#each shown as [name, count] (name)}
				{@const on = active.includes(name)}
				<li>
					<button
						type="button"
						onclick={(event) => onselect(name, event.ctrlKey || event.metaKey)}
						aria-pressed={on}
						title={on ? m.categories_clear() : m.categories_show({ name })}
						class="{sidebarItem(on)} justify-between"
					>
						<span class="truncate first-letter:uppercase">{name}</span>
						<span class="font-mono text-xs text-muted-foreground">{count}</span>
					</button>
				</li>
			{/each}
			{#if categories.length > TOP}
				<li>
					<button
						type="button"
						onclick={() => (expanded = !expanded)}
						class="{sidebarItem(false)} text-xs"
					>
						{expanded ? m.common_less() : m.categories_more({ count: categories.length - TOP })}
					</button>
				</li>
			{/if}
		</ul>
	</nav>
{/if}
