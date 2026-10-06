<script lang="ts" generics="V extends string">
	import * as Tabs from '#lib/components/ui/tabs/index.js';

	let {
		value = $bindable(),
		tabs
	}: {
		/** The section shown. */
		value: V;
		/** Each section, with how many it holds. */
		tabs: { value: V; label: string; count: number }[];
	} = $props();
</script>

<!-- A view's sections as tabs underlined on a rule across the column, each
     with how many it holds. What each shows is up to the view. -->
<Tabs.Root bind:value={() => value, (next) => (value = next as V)}>
	<Tabs.List
		variant="line"
		class="mb-5 h-auto w-full justify-start gap-5 border-b border-neutral-800 p-0"
	>
		{#each tabs as tab (tab.value)}
			<Tabs.Trigger
				value={tab.value}
				class="h-auto flex-none px-0 pt-1 pb-2.5 group-data-horizontal/tabs:after:-bottom-px"
			>
				{tab.label}
				<span class="text-xs font-normal text-muted-foreground tabular-nums">{tab.count}</span>
			</Tabs.Trigger>
		{/each}
	</Tabs.List>
</Tabs.Root>
