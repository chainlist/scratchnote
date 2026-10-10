<script lang="ts">
	import XIcon from '@lucide/svelte/icons/x';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Slider } from '#lib/components/ui/slider/index.js';
	import { dayOfNumber, shortDay } from '#lib/dates.js';
	import { m } from '#lib/paraglide/messages.js';
	import type { timeBars } from './map-data.js';

	let {
		extent,
		bars,
		span = $bindable(null)
	}: {
		/** The first and last day of the notes, as day numbers. */
		extent: readonly [number, number];
		/** How the notes spread over that time. */
		bars: ReturnType<typeof timeBars>;
		/** The first and last day shown, null while every day is. */
		span?: [number, number] | null;
	} = $props();
</script>

{#if extent[1] > extent[0]}
	{@const [from, to] = span ?? extent}
	<div class="flex flex-col gap-1.5" role="group" aria-label={m.map_dates()}>
		<div class="flex h-6 items-end gap-px" aria-hidden="true">
			{#each bars as bar, i (i)}
				<div
					class={[
						'flex-1 rounded-[1px]',
						bar.to > from && bar.from <= to ? 'bg-primary/60' : 'bg-muted-foreground/20'
					]}
					style:height="{bar.height ? Math.max(8, bar.height * 100) : 0}%"
				></div>
			{/each}
		</div>
		<Slider
			type="multiple"
			min={extent[0]}
			max={extent[1]}
			step={1}
			value={[from, to]}
			onValueChange={([first, last]) =>
				(span = first <= extent[0] && last >= extent[1] ? null : [first, last])}
			thumbLabel={(index) => (index === 0 ? m.map_dates_from() : m.map_dates_to())}
			valueText={(day) => shortDay(dayOfNumber(day))}
		/>
		<div class="flex h-5 items-center justify-between text-xs text-muted-foreground">
			<span>{shortDay(dayOfNumber(from))}</span>
			{#if span}
				<Button
					variant="ghost"
					size="icon-xs"
					aria-label={m.map_dates_all()}
					title={m.map_dates_all()}
					onclick={() => (span = null)}
				>
					<XIcon />
				</Button>
			{/if}
			<span>{shortDay(dayOfNumber(to))}</span>
		</div>
	</div>
{/if}
