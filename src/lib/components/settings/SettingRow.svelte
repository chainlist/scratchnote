<script lang="ts">
	import type { Snippet } from 'svelte';
	import { Label } from '$lib/components/ui/label';
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

<div class="flex items-center justify-between gap-6 px-4 py-3">
	<div class="flex flex-col gap-0.5">
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
