<script lang="ts">
	import type { Snippet } from 'svelte';
	import { m } from '#lib/paraglide/messages.js';

	let {
		error,
		children,
		actions
	}: {
		/** Why the plugin failed to load, if it did. */
		error?: string;
		/** Its name and what it does. */
		children: Snippet;
		/** Its options and its switch, at the row's end. */
		actions: Snippet;
	} = $props();
</script>

<!-- A plugin in the settings' lists. -->
<div class="flex items-center justify-between gap-6 px-4 py-3">
	<div class="flex min-w-0 flex-col gap-0.5">
		{@render children()}
		{#if error !== undefined}
			<p class="text-xs text-destructive">
				{m.plugins_failed_because({ error })}
			</p>
		{/if}
	</div>
	<div class="flex shrink-0 items-center gap-1">
		{@render actions()}
	</div>
</div>
