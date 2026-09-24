<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';

	let { value = $bindable() }: { value: string } = $props();
	let input = $state<HTMLInputElement | null>(null);
</script>

<div class="relative">
	<SearchIcon
		class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
	/>
	<!-- The webview's own clear button is hidden for the one below, which matches the app. -->
	<Input
		bind:ref={input}
		type="search"
		bind:value
		onkeydown={(e) => {
			if (e.key === 'Escape') value = '';
		}}
		placeholder="Search notes, #tag to filter"
		aria-label="Search notes"
		spellcheck="false"
		class="px-8 [&::-webkit-search-cancel-button]:appearance-none"
	/>
	{#if value}
		<Button
			variant="ghost"
			size="icon-xs"
			onclick={() => {
				value = '';
				input?.focus();
			}}
			aria-label="Clear search"
			class="absolute top-1/2 right-1.5 -translate-y-1/2 text-muted-foreground hover:text-foreground"
		>
			<XIcon />
		</Button>
	{/if}
</div>
