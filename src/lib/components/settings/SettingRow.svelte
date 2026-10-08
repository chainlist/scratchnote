<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Label } from '#lib/components/ui/label/index.js';
	import { hint as hintClass } from './styles';

	let {
		label,
		id,
		hint,
		children
	}: {
		label: string;
		/** The control's id, which makes the name a label for it. */
		id?: string;
		/** Under the name; a snippet when it needs markup. */
		hint: string | Snippet;
		/** The control, on the right. */
		children: Snippet;
	} = $props();
</script>

<!-- The control goes under its name when the two do not fit side by side. -->
<div class="flex flex-wrap items-center justify-between gap-x-6 gap-y-3 px-4 py-3">
	<div class="flex min-w-0 flex-1 basis-56 flex-col gap-0.5">
		{#if id}
			<Label for={id}>{label}</Label>
		{:else}
			<span class="text-sm font-medium">{label}</span>
		{/if}
		<p class={hintClass}>
			{#if typeof hint === 'string'}{hint}{:else}{@render hint()}{/if}
		</p>
	</div>
	{@render children()}
</div>
