<script lang="ts">
	import { m } from '#lib/paraglide/messages.js';

	let {
		detail,
		tight = false,
		class: className
	}: {
		/** The error as it came, for a report. */
		detail: string;
		/** Closer under its summary, as in a line of small text. */
		tight?: boolean;
		class?: string;
	} = $props();
</script>

<!-- The error as it came, folded under Details. -->
<details class={className}>
	<summary class="cursor-pointer">{m.error_details()}</summary>
	<p class={['font-mono break-all whitespace-pre-wrap select-text', tight ? 'mt-0.5' : 'mt-1']}>
		{detail}
	</p>
</details>
