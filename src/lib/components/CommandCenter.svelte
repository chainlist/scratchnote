<script lang="ts">
	import { tick, type Component } from 'svelte';
	import type { EditorView } from '@codemirror/view';
	import { search, searchMeaning, type Note } from '#lib/api.js';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import * as Command from '#lib/components/ui/command/index.js';
	import CalendarCheckIcon from '@lucide/svelte/icons/calendar-check';
	import FilePlusIcon from '@lucide/svelte/icons/file-plus';
	import FilesIcon from '@lucide/svelte/icons/files';
	import HashIcon from '@lucide/svelte/icons/hash';
	import ListIcon from '@lucide/svelte/icons/list';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import { categoryLabel } from '#lib/categories.js';
	import { m } from '#lib/paraglide/messages.js';
	import { formatHotkey, runCommand } from '#lib/plugins/commands.js';
	import { labelText, registry, type CommandEntry } from '#lib/plugins/registry.svelte.js';
	import { queryCategories } from '#lib/query.js';

	let {
		open = $bindable(),
		query = $bindable(),
		editor,
		categories,
		canChat,
		canMeaning,
		onpick,
		onseeall,
		ontoday,
		onnewpage,
		onpages,
		onchat,
		onsettings
	}: {
		open: boolean;
		/** Bound so a category clicked on a card can open the palette filtered on it. */
		query: string;
		/** The editor the palette was opened from, for the plugins' commands on text. */
		editor: EditorView | null;
		/**
		 * Every category in use with its count, most used first. Offered as
		 * filters while the query is empty, and as `#` completions.
		 */
		categories: [string, number][];
		canChat: boolean;
		/** Whether search by meaning can run: the embedding model is there and on. */
		canMeaning: boolean;
		/** Show a found note on its day. */
		onpick: (note: Note) => void;
		/** Show every note the query matches in the timeline. */
		onseeall: (query: string) => void;
		ontoday: () => void;
		/** Start a page on the day shown (SPEC 3.5). */
		onnewpage: () => void;
		/** List every page of the space. */
		onpages: () => void;
		onchat: () => void;
		onsettings: () => void;
	} = $props();

	/** Completions and results shown at once. */
	const CATEGORY_LIMIT = 6;
	const RESULT_LIMIT = 50;
	/** Embedding the query costs a model run, so it waits for typing to pause. */
	const MEANING_DELAY_MS = 300;

	let input = $state<HTMLInputElement | null>(null);
	let results = $state<Note[]>([]);
	/** How many notes the words match, of which `results` holds the first. */
	let total = $state(0);
	/** Notes close in meaning that the words miss, shown under the matches. */
	let related = $state<Note[]>([]);
	/** From a keystroke until the search by meaning for it has answered. */
	let meaningPending = $state(false);
	let error = $state<string | null>(null);

	const trimmed = $derived(query.trim());

	/** A command of the app's own, or a plugin's (SPEC 3.9). */
	type Action = {
		value: string;
		name: string;
		icon?: Component;
		plugin?: CommandEntry;
		run: () => void;
	};

	const builtIn = $derived<Action[]>([
		{ value: 'today', name: m.command_today(), icon: CalendarCheckIcon, run: ontoday },
		{ value: 'new-page', name: m.pages_new(), icon: FilePlusIcon, run: onnewpage },
		{ value: 'all-pages', name: m.pages_all(), icon: FilesIcon, run: onpages },
		...(canChat
			? [{ value: 'chat', name: m.search_chat_label(), icon: SparklesIcon, run: onchat }]
			: []),
		{ value: 'settings', name: m.common_settings(), icon: SettingsIcon, run: onsettings }
	]);

	/** The plugins' commands; those on text only when opened from an editor still there. */
	const pluginActions = $derived<Action[]>(
		registry.commands
			.filter((command) => command.callback || (command.editorCallback && editor?.dom.isConnected))
			.map((command) => ({
				value: `p:${command.id}`,
				name: labelText(command.name),
				plugin: command,
				run: () => {
					runCommand(command, editor);
					editor?.focus();
				}
			}))
	);

	/** Folded for matching: `é` finds `e`, and case does not count. */
	const fold = (text: string) =>
		text
			.normalize('NFD')
			.replace(/\p{Diacritic}/gu, '')
			.toLowerCase();

	/** The `#` token being typed at the end of the query, without the `#`. */
	const prefix = $derived.by(() => {
		if (query.endsWith(' ')) return null;
		const last = query.split(' ').pop() ?? '';
		return last.startsWith('#') ? last.slice(1).toLowerCase() : null;
	});

	/** While typing, the commands whose name has every word typed. */
	const matchingActions = $derived.by(() => {
		const words = fold(trimmed).split(/\s+/).filter(Boolean);
		if (words.length === 0 || prefix !== null) return [];
		return [...builtIn, ...pluginActions].filter((action) =>
			words.every((word) => fold(action.name).includes(word))
		);
	});

	// Categories already in the query are not offered again. Names starting
	// with the prefix come before names that only contain it, each most used
	// first. The name in the interface language matches too, so `#jeu` offers
	// `game`.
	const completions = $derived.by(() => {
		if (prefix === null) return [];
		const used = new Set(queryCategories(query));
		const starts: [string, number][] = [];
		const contains: [string, number][] = [];
		for (const entry of categories) {
			// The token being typed is in `used` too, so a complete one drops out.
			if (used.has(categoryLabel(entry[0]).toLowerCase())) continue;
			const names = [entry[0], categoryLabel(entry[0]).toLowerCase()];
			if (names.some((name) => name.startsWith(prefix))) starts.push(entry);
			else if (names.some((name) => name.includes(prefix))) contains.push(entry);
		}
		return [...starts, ...contains].slice(0, CATEGORY_LIMIT);
	});

	// Only the latest query's answer is kept, so a slow reply to an earlier
	// keystroke cannot overwrite a newer one.
	let searchRun = 0;
	$effect(() => {
		const q = trimmed;
		const run = ++searchRun;
		if (q === '') {
			results = [];
			total = 0;
			error = null;
			return;
		}
		search(q, RESULT_LIMIT)
			.then((found) => {
				if (run !== searchRun) return;
				results = found.notes;
				total = found.total;
				error = null;
			})
			.catch((e) => {
				if (run !== searchRun) return;
				results = [];
				total = 0;
				error = String(e);
			});
	});

	// Same guard as above, against a slow answer to an earlier pause.
	let meaningRun = 0;
	$effect(() => {
		const q = trimmed;
		const run = ++meaningRun;
		related = [];
		// Kept in a local: read back here, the state would make the answer
		// clearing it run this effect again, and search again.
		const pending = q !== '' && canMeaning;
		meaningPending = pending;
		if (!pending) return;
		const timer = setTimeout(() => {
			searchMeaning(q)
				.then((found) => {
					if (run === meaningRun) related = found;
				})
				// Search by words still works; this part is only extra.
				.catch(() => {})
				.finally(() => {
					if (run === meaningRun) meaningPending = false;
				});
		}, MEANING_DELAY_MS);
		return () => clearTimeout(timer);
	});

	// The caret goes after the query, so a filter opened from a category can be typed on.
	$effect(() => {
		if (!open) return;
		void tick().then(() => input?.setSelectionRange(query.length, query.length));
	});

	/** Put a category filter in the query, completing the `#` token if one is being typed. */
	function filter(name: string) {
		const words = query.split(' ');
		if (prefix !== null) words.pop();
		query = [...words.filter(Boolean), `#${categoryLabel(name)}`, ''].join(' ');
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

{#snippet actionItem(action: Action)}
	<Command.Item value={action.value} onSelect={() => run(action.run)}>
		{#if action.icon}<action.icon />{:else}<PluginIcon icon={action.plugin?.icon} />{/if}
		{action.name}
		{#if action.plugin?.hotkey}
			<Command.Shortcut>{formatHotkey(action.plugin.hotkey)}</Command.Shortcut>
		{/if}
	</Command.Item>
{/snippet}

{#snippet noteItem(note: Note)}
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
{/snippet}

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
				{#each builtIn as action (action.value)}
					{@render actionItem(action)}
				{/each}
			</Command.Group>
			{#if pluginActions.length > 0}
				<Command.Group heading={m.command_group_plugins()}>
					{#each pluginActions as action (action.value)}
						{@render actionItem(action)}
					{/each}
				</Command.Group>
			{/if}
			{#if categories.length > 0}
				<Command.Group heading={m.categories_heading()}>
					{#each categories as [name, count] (name)}
						<Command.Item value="c:{name}" onSelect={() => filter(name)}>
							<HashIcon />{categoryLabel(name)}
							<Command.Shortcut class="font-mono">{count}</Command.Shortcut>
						</Command.Item>
					{/each}
				</Command.Group>
			{/if}
		{:else}
			{#if completions.length > 0}
				<Command.Group heading={m.categories_heading()}>
					{#each completions as [name, count] (name)}
						<Command.Item value="c:{name}" onSelect={() => filter(name)}>
							<HashIcon />{categoryLabel(name)}
							<Command.Shortcut class="font-mono">{count}</Command.Shortcut>
						</Command.Item>
					{/each}
				</Command.Group>
			{/if}
			{#if results.length > 0}
				<Command.Group heading={m.page_results({ count: total })}>
					<!-- First, so Enter right after typing lists the matches as the old search did. -->
					<Command.Item value="see-all" onSelect={() => run(() => onseeall(trimmed))}>
						<ListIcon />{m.command_see_all()}
					</Command.Item>
					{#each results as note (note.id)}
						{@render noteItem(note)}
					{/each}
				</Command.Group>
			{/if}
			<!-- After the notes, so Enter still lists them when a word typed is
			     also in a command's name. -->
			{#if matchingActions.length > 0}
				<Command.Group heading={m.command_group_commands()}>
					{#each matchingActions as action (action.value)}
						{@render actionItem(action)}
					{/each}
				</Command.Group>
			{/if}
			{#if related.length > 0}
				<Command.Group heading={m.search_meaning_label()}>
					{#each related as note (note.id)}
						{@render noteItem(note)}
					{/each}
				</Command.Group>
			{:else if meaningPending}
				<!-- The embedding model at work looks like a note waiting for the
				     model: its words in the flowing colours of the glow. bits-ui
				     labels its loader "Loading..." in English; the label set after
				     its props says what is loading, in the interface language. -->
				<Command.Group heading={m.search_meaning_label()}>
					<Command.Loading>
						{#snippet child({ props })}
							<div
								{...props}
								aria-label={m.search_meaning_pending()}
								class="flex items-center gap-2 px-2 py-1.5 text-sm [&_svg]:size-4 [&_svg]:shrink-0"
							>
								<SparklesIcon class="text-violet-600 dark:text-violet-400" />
								<span
									data-text={m.search_meaning_pending()}
									class="note-glow-text relative isolate inline-block"
								>
									{m.search_meaning_pending()}
								</span>
							</div>
						{/snippet}
					</Command.Loading>
				</Command.Group>
			{/if}
			{#if error}
				<p class="px-3 py-2 text-sm text-red-400">{error}</p>
			{:else if !meaningPending}
				<!-- Only once both searches have answered: said earlier, it would
				     claim nothing matches while the one by meaning still looks. -->
				<Command.Empty>{m.page_no_match()}</Command.Empty>
			{/if}
		{/if}
	</Command.List>
</Command.Dialog>
