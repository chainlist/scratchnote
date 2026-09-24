<script lang="ts">
	import { tick } from 'svelte';
	import { search, type Note } from '$lib/api';
	import * as Command from '$lib/components/ui/command';
	import CalendarCheckIcon from '@lucide/svelte/icons/calendar-check';
	import HashIcon from '@lucide/svelte/icons/hash';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import { m } from '$lib/paraglide/messages';

	let {
		open = $bindable(),
		query = $bindable(),
		tags,
		categories,
		canChat,
		onpick,
		ontoday,
		onchat,
		onsettings
	}: {
		open: boolean;
		/** Bound so a tag clicked on a card can open the palette filtered on it. */
		query: string;
		/** Every tag with its count, most used first, offered as `#` completions. */
		tags: [string, number][];
		/** Offered as filters while the query is empty. */
		categories: [string, number][];
		canChat: boolean;
		/** Show a found note on its day. */
		onpick: (note: Note) => void;
		ontoday: () => void;
		onchat: () => void;
		onsettings: () => void;
	} = $props();

	/** Completions and results shown at once. */
	const TAG_LIMIT = 6;
	const RESULT_LIMIT = 50;

	let input = $state<HTMLInputElement | null>(null);
	let results = $state<Note[]>([]);
	let error = $state<string | null>(null);

	const trimmed = $derived(query.trim());

	/** The `#` token being typed at the end of the query, without the `#`. */
	const prefix = $derived.by(() => {
		if (query.endsWith(' ')) return null;
		const last = query.split(' ').pop() ?? '';
		return last.startsWith('#') ? last.slice(1).toLowerCase() : null;
	});

	// Tags already in the query are not offered again. Names starting with
	// the prefix come before names that only contain it, each most used first.
	const completions = $derived.by(() => {
		if (prefix === null) return [];
		const used = new Set(
			query
				.split(/\s+/)
				.filter((t) => t.startsWith('#'))
				.map((t) => t.slice(1).toLowerCase())
		);
		const starts: [string, number][] = [];
		const contains: [string, number][] = [];
		for (const entry of tags) {
			// The token being typed is in `used` too, so a complete one drops out.
			if (used.has(entry[0])) continue;
			if (entry[0].startsWith(prefix)) starts.push(entry);
			else if (entry[0].includes(prefix)) contains.push(entry);
		}
		return [...starts, ...contains].slice(0, TAG_LIMIT);
	});

	// Only the latest query's answer is kept, so a slow reply to an earlier
	// keystroke cannot overwrite a newer one.
	let searchRun = 0;
	$effect(() => {
		const q = trimmed;
		const run = ++searchRun;
		if (q === '') {
			results = [];
			error = null;
			return;
		}
		search(q)
			.then((found) => {
				if (run !== searchRun) return;
				results = found.slice(0, RESULT_LIMIT);
				error = null;
			})
			.catch((e) => {
				if (run !== searchRun) return;
				results = [];
				error = String(e);
			});
	});

	// The caret goes after the query, so a filter opened from a tag can be typed on.
	$effect(() => {
		if (!open) return;
		void tick().then(() => input?.setSelectionRange(query.length, query.length));
	});

	/** Put a tag filter in the query, completing the `#` token if one is being typed. */
	function filter(name: string) {
		const words = query.split(' ');
		if (prefix !== null) words.pop();
		query = [...words.filter(Boolean), `#${name}`, ''].join(' ');
		input?.focus();
	}

	function run(action: () => void) {
		open = false;
		action();
	}

	/** The first line of a body, which is what a result shows. */
	function firstLine(body: string) {
		return body.split('\n').find((line) => line.trim()) ?? '';
	}
</script>

<!-- The backend searches, so the palette shows what it gets back unfiltered. -->
<Command.Dialog
	bind:open
	shouldFilter={false}
	title={m.search_label()}
	description={m.search_placeholder()}
	class="top-[12%] sm:max-w-xl"
>
	<Command.Input bind:value={query} bind:ref={input} placeholder={m.search_placeholder()} />
	<Command.List class="max-h-[min(28rem,60vh)]">
		{#if trimmed === ''}
			<Command.Group heading={m.command_group_commands()}>
				<Command.Item value="today" onSelect={() => run(ontoday)}>
					<CalendarCheckIcon />{m.command_today()}
				</Command.Item>
				{#if canChat}
					<Command.Item value="chat" onSelect={() => run(onchat)}>
						<SparklesIcon />{m.search_chat_label()}
					</Command.Item>
				{/if}
				<Command.Item value="settings" onSelect={() => run(onsettings)}>
					<SettingsIcon />{m.common_settings()}
				</Command.Item>
			</Command.Group>
			{#if categories.length > 0}
				<Command.Group heading={m.categories_heading()}>
					{#each categories as [name, count] (name)}
						<Command.Item value="c:{name}" onSelect={() => filter(name)}>
							<HashIcon />{name}
							<Command.Shortcut class="font-mono">{count}</Command.Shortcut>
						</Command.Item>
					{/each}
				</Command.Group>
			{/if}
		{:else}
			{#if completions.length > 0}
				<Command.Group heading={m.search_tags_label()}>
					{#each completions as [name, count] (name)}
						<Command.Item value="t:{name}" onSelect={() => filter(name)}>
							<HashIcon />{name}
							<Command.Shortcut class="font-mono">{count}</Command.Shortcut>
						</Command.Item>
					{/each}
				</Command.Group>
			{/if}
			{#if results.length > 0}
				<Command.Group heading={m.page_results({ count: results.length })}>
					{#each results as note (note.id)}
						<Command.Item value="n:{note.id}" onSelect={() => run(() => onpick(note))}>
							<span class="w-24 shrink-0 self-start font-mono text-xs text-muted-foreground">
								{note.date}<span class="block">{note.time}</span>
							</span>
							<span class="flex min-w-0 flex-1 flex-col gap-0.5">
								<span class="truncate">{note.subject ?? firstLine(note.body)}</span>
								{#if note.subject}
									<span class="truncate text-xs text-muted-foreground">
										{firstLine(note.body)}
									</span>
								{/if}
							</span>
						</Command.Item>
					{/each}
				</Command.Group>
			{/if}
			{#if error}
				<p class="px-3 py-2 text-sm text-red-400">{error}</p>
			{:else}
				<Command.Empty>{m.page_no_match()}</Command.Empty>
			{/if}
		{/if}
	</Command.List>
</Command.Dialog>
