<script lang="ts">
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		count,
		onclick,
		ghost = false,
		class: className
	}: {
		/** How many are left to show, beside the label. */
		count?: number;
		onclick: () => void;
		/** A quiet button where the list ends, rather than an outlined one centred under it. */
		ghost?: boolean;
		/** The room above it. */
		class: string;
	} = $props();
</script>

<!-- A long list draws a stretch at a time; this draws the next. -->
{#if ghost}
	<Button variant="ghost" class="{className} text-muted-foreground" {onclick}>
		{m.search_show_more()}
	</Button>
{:else}
	<div class="{className} flex justify-center">
		<Button variant="outline" size="sm" {onclick}>
			{m.search_show_more()}
			{#if count !== undefined}
				<span class="text-xs text-muted-foreground tabular-nums">{count}</span>
			{/if}
		</Button>
	</div>
{/if}
