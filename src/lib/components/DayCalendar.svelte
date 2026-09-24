<script lang="ts">
	import CalendarIcon from '@lucide/svelte/icons/calendar';
	import { parseDate, type DateValue } from '@internationalized/date';
	import type { DaySummary } from '$lib/api';
	import { Button } from '$lib/components/ui/button';
	import * as Calendar from '$lib/components/ui/calendar';
	import * as Popover from '$lib/components/ui/popover';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';

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

	const locale = $derived(getLocale());
	const withNotes = $derived(new Set(days.map((day) => day.date)));
	const value = $derived(selected ? parseDate(selected) : undefined);

	function pick(date: DateValue | undefined) {
		if (!date) return;
		open = false;
		onselect(date.toString());
	}
</script>

<Popover.Root bind:open>
	<Popover.Trigger>
		{#snippet child({ props })}
			<Button
				{...props}
				variant="ghost"
				size="icon-sm"
				aria-label={m.calendar_pick()}
				title={m.calendar_pick()}
				class="text-muted-foreground hover:text-foreground"
			>
				<CalendarIcon />
			</Button>
		{/snippet}
	</Popover.Trigger>
	<Popover.Content class="w-auto bg-background p-0" align="start">
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
