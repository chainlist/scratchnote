<script lang="ts">
	import CalendarPage from '$lib/components/CalendarPage.svelte';
	import View from '$lib/components/View.svelte';
	import { m } from '$lib/paraglide/messages';
	import { getShell } from '$lib/shell.svelte';

	let { data } = $props();

	const shell = getShell();
</script>

<View back={shell.back} title={m.calendar_title()}>
	<CalendarPage
		days={data.days}
		selected={shell.day}
		onselect={(date) => void shell.openDay(date)}
	/>
</View>
