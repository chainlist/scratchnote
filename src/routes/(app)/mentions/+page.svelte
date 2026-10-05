<script lang="ts">
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { mentionHref } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();
</script>

<!-- Every name mentioned, most mentioned first, each with how many notes
     name it and the day of the last (SPEC 3.10). -->
<View back={shell.back} title={m.mentions_title()}>
	{#if data.mentions.length === 0}
		<p class="text-base text-neutral-600">{m.mentions_none()}</p>
	{:else}
		<ul>
			{#each data.mentions as mention (mention.key)}
				<li>
					<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
					<a
						href={mentionHref(mention.name)}
						class="-mx-2 flex items-baseline justify-between gap-4 rounded px-2 py-2 hover:bg-neutral-900"
					>
						<span class="mention truncate">@{mention.name}</span>
						<span class="shrink-0 text-xs text-neutral-500">
							{m.thread_last({ count: mention.notes, date: shortDay(mention.last) })}
						</span>
					</a>
				</li>
			{/each}
		</ul>
	{/if}
</View>
