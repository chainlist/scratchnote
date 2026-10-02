<script lang="ts">
	import { page } from '$app/state';
	import View from '#lib/components/View.svelte';
	import { getShell } from '#lib/shell.svelte.js';

	const shell = getShell();
</script>

<!-- A view that could not load, in its place; the rest of the window works. -->
<View back={shell.back}>
	<p
		class="rounded border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
	>
		{page.error?.message}
	</p>
</View>
