<script lang="ts">
	import { invalidateAll } from '$app/navigation';
	import { page } from '$app/state';
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import View from '#lib/components/View.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	const shell = getShell();

	let retrying = $state(false);

	/** Load the view again, as a file held by another program is often
	 *  free a moment later. */
	async function retry() {
		retrying = true;
		await invalidateAll();
		retrying = false;
	}
</script>

<!-- A view that could not load, in its place; the rest of the window works.
     What failed in the app's words, the error as it came under Details. -->
<View back={shell.back}>
	<div role="alert" class="flex flex-col items-center gap-3 py-16 text-center">
		<p class="flex items-center gap-2 text-base text-neutral-200">
			<CircleAlertIcon class="size-4 shrink-0 text-destructive" />{m.error_load_view()}
		</p>
		{#if page.error?.message}
			<details class="max-w-full text-xs text-meta">
				<summary class="cursor-pointer">{m.error_details()}</summary>
				<p class="mt-1 font-mono break-all whitespace-pre-wrap select-text">
					{page.error.message}
				</p>
			</details>
		{/if}
		<!-- Nothing to try again for a view that does not exist. -->
		{#if page.status !== 404}
			<Button variant="outline" size="sm" onclick={retry} disabled={retrying}>
				{m.error_retry()}
			</Button>
		{/if}
	</div>
</View>
