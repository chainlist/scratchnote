<script lang="ts">
	import MentionChip from '#lib/components/common/MentionChip.svelte';
	import { mentionHue } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { NO_NAME } from './map-data.js';

	let {
		entries,
		picked,
		onpoint,
		onpick,
		nameOf
	}: {
		/** Each name with its number of notes, `NO_NAME` for the notes that mention none. */
		entries: { key: string; count: number }[];
		/** The name whose notes alone are drawn as ever, null while every note is. */
		picked: string | null;
		/** A name pointed at, whose notes are brought forward, or none any more. */
		onpoint: (key: string | null) => void;
		onpick: (key: string) => void;
		nameOf: (key: string) => string;
	} = $props();
</script>

<!-- Pointing at a name brings its notes forward; clicking it shows
     only them, until it is clicked again. -->
<ul class="-mx-1 space-y-px">
	{#each entries as { key, count } (key)}
		<li>
			<button
				type="button"
				class={[
					'flex w-full items-center gap-2 rounded-md px-1 py-0.5 text-left transition-colors hover:bg-muted',
					picked === key && 'bg-muted'
				]}
				style:--hue={key === NO_NAME ? undefined : mentionHue(key)}
				aria-pressed={picked === key}
				onclick={() => onpick(key)}
				onpointerenter={() => onpoint(key)}
				onpointerleave={() => onpoint(null)}
				onfocus={() => onpoint(key)}
				onblur={() => onpoint(null)}
			>
				{#if key === NO_NAME}
					<span class="min-w-0 flex-1 truncate text-muted-foreground">{m.map_no_name()}</span>
				{:else}
					<span class="min-w-0 flex-1 truncate">
						<MentionChip class="text-sm">{nameOf(key)}</MentionChip>
					</span>
				{/if}
				<span class="text-xs text-muted-foreground tabular-nums">{count}</span>
			</button>
		</li>
	{/each}
</ul>
