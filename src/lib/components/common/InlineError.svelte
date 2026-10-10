<script lang="ts">
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import ErrorDetails from '#lib/components/common/ErrorDetails.svelte';
	import { cn } from '#lib/utils.js';

	let {
		message,
		detail,
		class: className
	}: {
		/** What failed, in the app's words. */
		message: string;
		/** The error as it came, under Details, for a report. */
		detail?: string | null;
		class?: string;
	} = $props();
</script>

<!-- A failure shown where it happened, under the editor or the note it
     belongs to, read out as it appears. -->
<div role="alert" class={cn('flex items-start gap-1.5 text-xs text-destructive', className)}>
	<CircleAlertIcon class="mt-px size-3.5 shrink-0" />
	<div class="min-w-0">
		<p>{message}</p>
		{#if detail}
			<ErrorDetails class="mt-0.5 text-meta" tight {detail} />
		{/if}
	</div>
</div>
