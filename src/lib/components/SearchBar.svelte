<script lang="ts">
	import { Button } from '$lib/components/ui/button';
	import { Input } from '$lib/components/ui/input';
	import SearchIcon from '@lucide/svelte/icons/search';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '$lib/paraglide/messages';

	let {
		value = $bindable(),
		tags,
		canChat,
		modelOff,
		onchat
	}: {
		value: string;
		/** Every tag with its count, most used first, offered as `#` completions. */
		tags: [string, number][];
		/** Whether a model is there to talk to. */
		canChat: boolean;
		/** The model is switched off in settings, which is why chat is not there. */
		modelOff: boolean;
		/** Open the chat about this space's notes. */
		onchat: () => void;
	} = $props();

	/** Completions shown at once. */
	const LIMIT = 8;

	let input = $state<HTMLInputElement | null>(null);
	let caret = $state(0);
	let highlighted = $state(0);
	// Escape hides the list without clearing the query; typing brings it back.
	let dismissed = $state(false);

	/** The `#` token the caret is in, as its start, end and text after the `#`. */
	const token = $derived.by(() => {
		const start = value.lastIndexOf(' ', caret - 1) + 1;
		const next = value.indexOf(' ', caret);
		const end = next === -1 ? value.length : next;
		const text = value.slice(start, end);
		return text.startsWith('#') ? { start, end, prefix: text.slice(1).toLowerCase() } : null;
	});

	// Tags already in the query are not offered again. Names starting with
	// the prefix come before names that only contain it, each most used first.
	const suggestions = $derived.by(() => {
		if (!token || dismissed) return [];
		const used = new Set(
			value
				.split(/\s+/)
				.filter((t) => t.startsWith('#'))
				.map((t) => t.slice(1).toLowerCase())
		);
		const starts: [string, number][] = [];
		const contains: [string, number][] = [];
		for (const entry of tags) {
			const name = entry[0];
			// The token being typed is in `used` too, so a complete one drops out.
			if (used.has(name)) continue;
			if (name.startsWith(token.prefix)) starts.push(entry);
			else if (name.includes(token.prefix)) contains.push(entry);
		}
		return [...starts, ...contains].slice(0, LIMIT);
	});

	$effect(() => {
		void suggestions;
		highlighted = 0;
	});

	function track() {
		caret = input?.selectionStart ?? value.length;
	}

	function complete(tag: string) {
		if (!token) return;
		const after = value.slice(token.end).replace(/^ /, '');
		const before = value.slice(0, token.start);
		value = `${before}#${tag} ${after}`;
		const at = before.length + tag.length + 2;
		caret = at;
		// Set after Svelte writes the new value into the field.
		queueMicrotask(() => input?.setSelectionRange(at, at));
	}

	function onkeydown(event: KeyboardEvent) {
		if (suggestions.length > 0) {
			if (event.key === 'ArrowDown' || event.key === 'ArrowUp') {
				event.preventDefault();
				const step = event.key === 'ArrowDown' ? 1 : -1;
				highlighted = (highlighted + step + suggestions.length) % suggestions.length;
				return;
			}
			if (event.key === 'Enter' || event.key === 'Tab') {
				event.preventDefault();
				complete(suggestions[highlighted][0]);
				return;
			}
			if (event.key === 'Escape') {
				dismissed = true;
				return;
			}
		}
		if (event.key === 'Escape') value = '';
	}
</script>

<div class="relative">
	<SearchIcon
		class="pointer-events-none absolute top-1/2 left-2.5 size-4 -translate-y-1/2 text-muted-foreground"
	/>
	<!-- The webview's own clear button is hidden for the one below, which matches the app. -->
	<Input
		bind:ref={input}
		type="search"
		bind:value
		{onkeydown}
		oninput={() => {
			dismissed = false;
			track();
		}}
		onclick={track}
		onkeyup={track}
		onfocus={track}
		onblur={() => (dismissed = true)}
		placeholder={m.search_placeholder()}
		aria-label={m.search_label()}
		aria-autocomplete="list"
		aria-controls="tag-suggestions"
		aria-expanded={suggestions.length > 0}
		spellcheck="false"
		class="pr-15 pl-8 [&::-webkit-search-cancel-button]:appearance-none"
	/>
	<div class="absolute top-1/2 right-1.5 flex -translate-y-1/2 items-center gap-0.5">
		{#if value}
			<Button
				variant="ghost"
				size="icon-xs"
				onclick={() => {
					value = '';
					input?.focus();
				}}
				aria-label={m.search_clear()}
				class="text-muted-foreground hover:text-foreground"
			>
				<XIcon />
			</Button>
		{/if}
		<Button
			variant="ghost"
			size="icon-xs"
			onclick={onchat}
			disabled={!canChat}
			aria-label={m.search_chat_label()}
			title={canChat
				? m.search_chat_title()
				: modelOff
					? m.search_chat_model_off()
					: m.search_chat_disabled()}
			class="text-muted-foreground hover:text-foreground"
		>
			<SparklesIcon />
		</Button>
	</div>

	{#if suggestions.length > 0}
		<ul
			id="tag-suggestions"
			role="listbox"
			aria-label={m.search_tags_label()}
			class="absolute inset-x-0 top-full z-20 mt-1 overflow-hidden rounded-md border bg-popover p-1 text-popover-foreground shadow-md"
		>
			{#each suggestions as [tag, count], i (tag)}
				<li role="option" aria-selected={i === highlighted}>
					<!-- mousedown, not click, so the field keeps focus and its caret. -->
					<button
						type="button"
						tabindex="-1"
						onmousedown={(event) => {
							event.preventDefault();
							complete(tag);
						}}
						onmouseenter={() => (highlighted = i)}
						class={[
							'flex h-7 w-full items-center justify-between rounded-sm px-2 text-left text-sm',
							i === highlighted ? 'bg-accent text-accent-foreground' : 'text-muted-foreground'
						]}
					>
						<span class="truncate">#{tag}</span>
						<span class="font-mono text-xs text-muted-foreground">{count}</span>
					</button>
				</li>
			{/each}
		</ul>
	{/if}
</div>
