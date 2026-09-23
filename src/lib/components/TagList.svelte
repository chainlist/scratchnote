<script lang="ts">
	let {
		tags,
		active,
		onselect
	}: {
		tags: [string, number][];
		/** Tags currently filtered on, highlighted in the list. */
		active: string[];
		onselect: (tag: string) => void;
	} = $props();
</script>

{#if tags.length > 0}
	<section>
		<h2 class="mb-1 px-2 text-[11px] font-medium tracking-wide text-neutral-500 uppercase">Tags</h2>
		<ul>
			{#each tags as [tag, count] (tag)}
				<li>
					<button
						type="button"
						onclick={() => onselect(tag)}
						class="flex w-full items-baseline justify-between rounded px-2 py-1 text-left text-sm
							{active.includes(tag)
							? 'bg-neutral-800 text-neutral-100'
							: 'text-neutral-400 hover:bg-neutral-900'}"
					>
						<span class="truncate">#{tag}</span>
						<span class="font-mono text-xs text-neutral-600">{count}</span>
					</button>
				</li>
			{/each}
		</ul>
	</section>
{/if}
