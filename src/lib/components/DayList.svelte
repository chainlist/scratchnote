<script lang="ts">
	import type { DaySummary } from '$lib/api';

	let {
		days,
		selected,
		onselect
	}: {
		days: DaySummary[];
		selected: string;
		onselect: (date: string) => void;
	} = $props();

	const monthOf = (date: string) => date.slice(0, 7);

	const monthLabel = (month: string) =>
		new Date(`${month}-01T00:00:00`).toLocaleDateString(undefined, {
			month: 'long',
			year: 'numeric'
		});

	const dayLabel = (date: string) =>
		new Date(`${date}T00:00:00`).toLocaleDateString(undefined, {
			weekday: 'short',
			day: 'numeric'
		});

	// Days arrive newest first; group runs of the same month without resorting.
	let months = $derived(
		days.reduce<{ month: string; days: DaySummary[] }[]>((groups, day) => {
			const month = monthOf(day.date);
			const last = groups.at(-1);
			if (last?.month === month) last.days.push(day);
			else groups.push({ month, days: [day] });
			return groups;
		}, [])
	);
</script>

<nav class="flex flex-col gap-4">
	{#each months as group (group.month)}
		<section>
			<h2 class="mb-1 px-2 text-[11px] font-medium tracking-wide text-neutral-500 uppercase">
				{monthLabel(group.month)}
			</h2>
			<ul>
				{#each group.days as day (day.date)}
					<li>
						<button
							type="button"
							onclick={() => onselect(day.date)}
							class="flex w-full items-baseline justify-between rounded px-2 py-1 text-left text-sm
								{day.date === selected
								? 'bg-neutral-800 text-neutral-100'
								: 'text-neutral-400 hover:bg-neutral-900'}"
						>
							<span>{dayLabel(day.date)}</span>
							<span class="font-mono text-xs text-neutral-600">{day.count}</span>
						</button>
					</li>
				{/each}
			</ul>
		</section>
	{:else}
		<p class="px-2 text-sm text-neutral-600">No notes yet.</p>
	{/each}
</nav>
