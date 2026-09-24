<script lang="ts">
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import { parseDate, type DateValue } from '@internationalized/date';
	import type { DaySummary } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import * as Calendar from '$lib/components/ui/calendar';
	import * as Popover from '$lib/components/ui/popover';

	let {
		days,
		selected,
		onselect
	}: {
		days: DaySummary[];
		selected: string;
		onselect: (date: string) => void;
	} = $props();

	let open = $state(false);

	const locale = Intl.DateTimeFormat().resolvedOptions().locale;
	const withNotes = $derived(new Set(days.map((day) => day.date)));
	const value = $derived(selected ? parseDate(selected) : undefined);
	const label = $derived(
		selected
			? new Date(`${selected}T00:00:00`).toLocaleDateString(undefined, {
					weekday: 'short',
					day: 'numeric',
					month: 'short',
					year: 'numeric'
				})
			: 'Pick a day'
	);

	function pick(date: DateValue | undefined) {
		if (!date) return;
		open = false;
		onselect(date.toString());
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button {...props} variant="outline" class="w-full justify-start font-normal">
				<CalendarIcon />
				{label}
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-auto p-0" align="start">
		<Calendar.Calendar
			type="single"
			{value}
			onValueChange={pick}
			preventDeselect
			{locale}
			class="bg-transparent"
		>
			{#snippet day({ day })}
				<Calendar.Day>
					{#snippet children({ day: number })}
						{number}
						<span
							class="size-1 rounded-full bg-current {withNotes.has(day.toString())
								? ''
								: 'invisible'}"
						></span>
					{/snippet}
				</Calendar.Day>
			{/snippet}
		</Calendar.Calendar>
	</Popover.Content>
</Popover.Root>
