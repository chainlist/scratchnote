<script lang="ts">
	let { value = $bindable() }: { value: string } = $props();
</script>

<input
	type="search"
	bind:value
	onkeydown={(e) => {
		if (e.key === 'Escape') value = '';
	}}
	placeholder="Search notes, #tag to filter"
	aria-label="Search notes"
	spellcheck="false"
	class="w-full rounded-md border border-neutral-800 bg-neutral-900 px-3 py-1.5 text-sm text-neutral-100 placeholder:text-neutral-600 focus:border-neutral-600 focus:outline-none"
/>
