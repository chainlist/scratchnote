<script lang="ts">
	import { dayTitle } from '#lib/dates.js';

	/** A day's name as its column heads it: the weekday, then the date, quieter. */
	let { date }: { date: string } = $props();

	const title = $derived(dayTitle(date));
</script>

{title.weekday} <span class="font-normal whitespace-nowrap text-meta">{title.date}</span>
