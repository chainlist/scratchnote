<script lang="ts">
	import type { IndexEntry } from '$lib/api';
	import Chat from '$lib/components/Chat.svelte';
	import { Button } from '$lib/components/ui/button';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '$lib/paraglide/messages';

	let {
		open = $bindable(),
		space,
		canChat,
		modelOff,
		docked = false,
		onopen
	}: {
		open: boolean;
		/** The open space, whose index the chat reads. */
		space: string;
		canChat: boolean;
		/** The model is switched off in settings, which is why chat is not on offer. */
		modelOff: boolean;
		/** A page is docked on the right, so the chat floats beside it, over the day. */
		docked?: boolean;
		/** Show a cited note on its day. */
		onopen: (entry: IndexEntry) => void;
	} = $props();
</script>

<!-- The chat about the open space's index, floating over the day. -->
{#if open}
	<section
		aria-label={m.chat_title()}
		class={[
			'fixed bottom-16 z-40 flex h-[min(40rem,calc(100vh-7rem))] flex-col overflow-hidden rounded-xl border bg-popover text-popover-foreground shadow-2xl',
			docked
				? 'right-[calc(var(--page-dock)+1.5rem)] w-[min(26rem,calc(100vw-var(--page-dock)-3rem))]'
				: 'right-6 w-[min(26rem,calc(100vw-3rem))]'
		]}
	>
		<!-- Another space has another index, so a switch starts a new chat. -->
		{#key space}
			<Chat {space} onclose={() => (open = false)} {onopen} />
		{/key}
	</section>
{/if}

<!-- A disabled button shows no title, so the wrapper carries it. -->
<span
	class={['fixed bottom-2 z-40', docked ? 'right-[calc(var(--page-dock)+1.5rem)]' : 'right-6']}
	title={open
		? m.common_close()
		: canChat
			? m.search_chat_title()
			: modelOff
				? m.search_chat_model_off()
				: m.search_chat_disabled()}
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
