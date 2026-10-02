<script lang="ts">
	import { onMount, tick } from 'svelte';
	import {
		chat,
		splitCitations,
		stopChat,
		warmChat,
		type ChatMessage,
		type IndexEntry
	} from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import ArrowUpIcon from '@lucide/svelte/icons/arrow-up';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import SquareIcon from '@lucide/svelte/icons/square';
	import SquarePenIcon from '@lucide/svelte/icons/square-pen';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '#lib/paraglide/messages.js';

	let {
		space,
		onclose,
		onopen
	}: {
		/** The open space, whose index the model reads. */
		space: string;
		onclose: () => void;
		/** Open the day a cited note is on. */
		onopen: (entry: IndexEntry) => void;
	} = $props();

	interface Turn extends ChatMessage {
		/** Note numbers the reply cites, once it is done. */
		cited?: number[];
		failed?: string;
	}

	// Sent as typed, so the model answers in the language they are in.
	const SUGGESTIONS = [m.chat_suggestion_week, m.chat_suggestion_watched, m.chat_suggestion_todo];

	let turns = $state<Turn[]>([]);
	/** The index as the model numbers it: note n is notes[n - 1]. */
	let notes = $state<IndexEntry[]>([]);
	let draft = $state('');
	let replying = $state(false);
	let warming = $state(false);
	let error = $state<string | null>(null);
	let scroller = $state<HTMLElement | null>(null);
	let composer = $state<HTMLTextAreaElement | null>(null);

	// Only the latest reply writes into the conversation.
	let run = 0;

	onMount(() => {
		composer?.focus();
		void warm();
		return () => {
			run++;
			void stopChat();
		};
	});

	/** The first reply would otherwise wait while the model reads the whole index. */
	async function warm() {
		warming = true;
		try {
			await warmChat();
		} catch (e) {
			error = String(e);
		} finally {
			warming = false;
		}
	}

	async function send(text = draft) {
		const content = text.trim();
		if (!content || replying) return;
		draft = '';
		error = null;
		turns.push({ role: 'user', content });
		// A failed or empty reply is not part of the conversation.
		const history: ChatMessage[] = turns
			.filter((t) => t.role === 'user' || (t.content.trim() && !t.failed))
			.map(({ role, content }) => ({ role, content }));
		turns.push({ role: 'assistant', content: '' });
		const reply = turns[turns.length - 1];
		const mine = ++run;
		replying = true;
		await scrollDown();
		try {
			await chat(history, (event) => {
				if (mine !== run) return;
				if (event.kind === 'notes') notes = event.notes;
				else if (event.kind === 'token') {
					reply.content += event.text;
					void scrollDown();
				} else reply.cited = event.cited;
			});
		} catch (e) {
			if (mine === run) reply.failed = String(e);
		} finally {
			if (mine === run) {
				replying = false;
				await tick();
				composer?.focus();
			}
		}
	}

	function stop() {
		void stopChat();
	}

	function restart() {
		if (replying) {
			run++;
			replying = false;
			void stopChat();
		}
		turns = [];
		draft = '';
		error = null;
		composer?.focus();
	}

	async function scrollDown() {
		await tick();
		scroller?.scrollTo({ top: scroller.scrollHeight });
	}

	function onkeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && !event.shiftKey && !event.isComposing) {
			event.preventDefault();
			void send();
		}
	}

	type Block =
		| { kind: 'text'; text: string }
		| { kind: 'heading'; text: string }
		| { kind: 'list'; ordered: boolean; items: string[] };

	/**
	 * A reply's lines grouped into paragraphs, headings and lists: the
	 * markdown a small model writes unasked, which would otherwise show as
	 * raw `-` and `#`. A blank line inside a list does not end it.
	 */
	function blocks(text: string): Block[] {
		const out: Block[] = [];
		for (const line of text.split('\n')) {
			const last = out.at(-1);
			const item = line.match(/^\s*(?:[-*•]|(\d+)[.)])\s+(.*)$/);
			const heading = line.match(/^\s*#{1,6}\s+(.*)$/);
			if (item) {
				const ordered = item[1] !== undefined;
				if (last?.kind === 'list' && last.ordered === ordered) last.items.push(item[2]);
				else out.push({ kind: 'list', ordered, items: [item[2]] });
			} else if (heading) out.push({ kind: 'heading', text: heading[1] });
			else if (last?.kind === 'text') last.text += `\n${line}`;
			else if (line.trim() || last?.kind !== 'list') out.push({ kind: 'text', text: line });
		}
		return out
			.map((block) => (block.kind === 'text' ? { ...block, text: block.text.trim() } : block))
			.filter((block) => block.kind !== 'text' || block.text);
	}

	/** `**bold**`, the one bit of markdown a short reply leans on. */
	function bold(text: string): { text: string; strong: boolean }[] {
		return text
			.split(/\*\*(.+?)\*\*/g)
			.map((part, i) => ({ text: part, strong: i % 2 === 1 }))
			.filter((part) => part.text);
	}

	function label(entry: IndexEntry) {
		return entry.subject ?? `${entry.date} ${entry.time}`;
	}
</script>

{#snippet inline(text: string, cursor: boolean)}
	{#each splitCitations(text) as part, j (j)}
		{#if typeof part === 'string'}
			{#each bold(part) as piece, k (k)}
				{#if piece.strong}<strong class="font-semibold">{piece.text}</strong
					>{:else}{piece.text}{/if}
			{/each}
		{:else}
			{#each part as n (n)}
				{@const entry = notes[n - 1]}
				{#if entry}
					<button
						type="button"
						onclick={() => onopen(entry)}
						title="{entry.date} {entry.time}: {label(entry)}"
						class="mx-0.5 inline-flex max-w-56 cursor-pointer items-baseline rounded bg-primary/10 px-1.5 align-baseline font-mono text-xs text-primary hover:bg-primary/20"
					>
						<span class="truncate">{label(entry)}</span>
					</button>
				{:else}[{n}]{/if}
			{/each}
		{/if}
	{/each}
	{#if cursor}<span
			class="ml-0.5 inline-block h-4 w-1.5 animate-pulse bg-neutral-400 align-text-bottom"
		></span>{/if}
{/snippet}

<!-- Esc closes the panel from anywhere inside it, as it would a dialog. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="flex min-h-0 flex-1 flex-col"
	onkeydown={(event) => {
		if (event.key === 'Escape' && !event.defaultPrevented) onclose();
	}}
>
	<header class="flex items-center gap-2 border-b py-2 pr-2 pl-4">
		<SparklesIcon class="size-4 text-primary" />
		<h2 class="min-w-0 flex-1 truncate text-base font-semibold">
			{m.chat_title()}
			<span class="font-normal text-muted-foreground">{m.chat_about({ space })}</span>
		</h2>
		{#if turns.length > 0}
			<Button variant="ghost" size="sm" onclick={restart} class="text-muted-foreground">
				<SquarePenIcon />
				{m.chat_new()}
			</Button>
		{/if}
		<Button
			variant="ghost"
			size="icon-sm"
			onclick={onclose}
			aria-label={m.common_close()}
			title={m.common_close()}
			class="text-muted-foreground hover:text-foreground"
		>
			<XIcon />
		</Button>
	</header>

	<div bind:this={scroller} class="min-h-0 flex-1 overflow-y-auto px-4">
		<div class="flex flex-col gap-5 py-4">
			{#if turns.length === 0}
				<div class="flex flex-col gap-4 text-sm text-muted-foreground">
					<p>{m.chat_intro({ space })}</p>
					{#if warming}
						<p class="animate-pulse text-xs">{m.chat_reading_space({ space })}</p>
					{/if}
					<div class="flex flex-wrap gap-2">
						{#each SUGGESTIONS as suggestion (suggestion)}
							<Button variant="outline" size="sm" onclick={() => send(suggestion())}>
								{suggestion()}
							</Button>
						{/each}
					</div>
				</div>
			{/if}

			{#each turns as turn, i (i)}
				{#if turn.role === 'user'}
					<p
						class="max-w-[85%] self-end rounded-2xl rounded-br-sm bg-muted px-4 py-2 text-[0.9375rem] leading-6 whitespace-pre-wrap"
					>
						{turn.content}
					</p>
				{:else}
					<div class="flex flex-col gap-3">
						{#if turn.content}
							{@const laid = blocks(turn.content)}
							{@const streaming = replying && i === turns.length - 1}
							<div class="flex flex-col gap-2 text-[0.9375rem] leading-7 text-neutral-200">
								{#each laid as block, b (b)}
									{@const end = streaming && b === laid.length - 1}
									{#if block.kind === 'list'}
										<svelte:element
											this={block.ordered ? 'ol' : 'ul'}
											class="flex flex-col gap-1 pl-5 {block.ordered
												? 'list-decimal'
												: 'list-disc'}"
										>
											{#each block.items as item, k (k)}
												<li>{@render inline(item, end && k === block.items.length - 1)}</li>
											{/each}
										</svelte:element>
									{:else if block.kind === 'heading'}
										<p class="font-semibold">{@render inline(block.text, end)}</p>
									{:else}
										<p class="whitespace-pre-wrap">{@render inline(block.text, end)}</p>
									{/if}
								{/each}
							</div>
						{:else if replying && i === turns.length - 1}
							<p class="animate-pulse text-sm text-muted-foreground">
								{warming ? m.chat_reading_first() : m.chat_thinking()}
							</p>
						{/if}

						{#if turn.failed}
							<p
								class="rounded border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
							>
								{turn.failed}
							</p>
						{/if}

						{#if turn.cited && turn.cited.length > 0}
							<ul class="flex flex-col border-l border-neutral-800 pl-3">
								{#each turn.cited as n (n)}
									{@const entry = notes[n - 1]}
									{#if entry}
										<li>
											<button
												type="button"
												onclick={() => onopen(entry)}
												class="flex w-full cursor-pointer items-baseline gap-3 rounded px-2 py-1 text-left text-sm hover:bg-neutral-900"
											>
												<span class="shrink-0 font-mono text-xs text-neutral-600">{entry.date}</span
												>
												<span class="min-w-0 truncate text-neutral-300">{label(entry)}</span>
											</button>
										</li>
									{/if}
								{/each}
							</ul>
						{/if}
					</div>
				{/if}
			{/each}

			{#if error}
				<p
					class="rounded border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
				>
					{error}
				</p>
			{/if}
		</div>
	</div>

	<div class="px-3 pb-3">
		<div
			class="flex items-end gap-2 rounded-xl border bg-muted/40 p-2 focus-within:border-neutral-600"
		>
			<textarea
				bind:this={composer}
				bind:value={draft}
				{onkeydown}
				rows="1"
				placeholder={m.chat_placeholder()}
				aria-label={m.chat_message_label()}
				class="field-sizing-content max-h-40 min-h-9 flex-1 resize-none bg-transparent px-2 py-1.5 text-[0.9375rem] leading-6 outline-none placeholder:text-muted-foreground"
			></textarea>
			{#if replying}
				<Button
					size="icon-sm"
					variant="outline"
					onclick={stop}
					aria-label={m.chat_stop()}
					title={m.chat_stop()}
				>
					<SquareIcon />
				</Button>
			{:else}
				<Button
					size="icon-sm"
					onclick={() => send()}
					disabled={!draft.trim()}
					aria-label={m.chat_send()}
					title={m.chat_send_title()}
				>
					<ArrowUpIcon />
				</Button>
			{/if}
		</div>
	</div>
</div>
