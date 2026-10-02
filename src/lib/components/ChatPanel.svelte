<script lang="ts">
	import type { IndexEntry } from '#lib/api.js';
	import Chat from '#lib/components/Chat.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '#lib/paraglide/messages.js';

	let {
		open = $bindable(),
		space,
		canChat,
		onopen
	}: {
		open: boolean;
		/** The open space, whose index the chat reads. */
		space: string;
		canChat: boolean;
		/** Show a cited note on its day. */
		onopen: (entry: IndexEntry) => void;
	} = $props();
</script>

<!-- The chat about the open space's index, floating over the day. It sits in
     the view's pane, so it stays clear of the dock on either side. -->
{#if open}
	<section
		aria-label={m.chat_title()}
		class="absolute right-6 bottom-16 z-40 flex h-[min(40rem,calc(100%-5rem))] w-[min(26rem,calc(100%-3rem))] flex-col overflow-hidden rounded-xl border bg-popover text-popover-foreground shadow-2xl"
	>
		<!-- Another space has another index, so a switch starts a new chat. -->
		{#key space}
			<Chat {space} onclose={() => (open = false)} {onopen} />
		{/key}
	</section>
{/if}

<!-- A disabled button shows no title, so the wrapper carries it. -->
<span
	class="absolute right-6 bottom-2 z-40"
	title={open ? m.common_close() : canChat ? m.search_chat_title() : m.search_chat_disabled()}
>
	<Button
		onclick={() => (open = !open)}
		disabled={!canChat && !open}
		aria-label={open ? m.common_close() : m.search_chat_label()}
		aria-expanded={open}
		class="size-12 rounded-full shadow-lg [&_svg:not([class*='size-'])]:size-5"
	>
		{#if open}<XIcon />{:else}<SparklesIcon />{/if}
	</Button>
</span>
