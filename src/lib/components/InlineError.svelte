<script lang="ts">
	import CircleAlertIcon from '@lucide/svelte/icons/circle-alert';
	import { m } from '#lib/paraglide/messages.js';
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
			<details class="mt-0.5 text-meta">
				<summary class="cursor-pointer">{m.error_details()}</summary>
				<p class="mt-0.5 font-mono break-all whitespace-pre-wrap select-text">{detail}</p>
			</details>
		{/if}
	</div>
</div>
