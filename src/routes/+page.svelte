<script lang="ts">
	import { onMount } from 'svelte';
	import {
		deleteNote,
		getDay,
		listDays,
		listTags,
		modelStatus,
		onIndexRebuilt,
		onModelStatus,
		onNoteEnriched,
		onNoteUpdated,
		search,
		today,
		type DaySummary,
		type ModelStatus,
		type Note
	} from '$lib/api';
	import DayList from '$lib/components/DayList.svelte';
	import NoteCard from '$lib/components/NoteCard.svelte';
	import Onboarding from '$lib/components/Onboarding.svelte';
	import SearchBar from '$lib/components/SearchBar.svelte';
	import TagList from '$lib/components/TagList.svelte';

	let days = $state<DaySummary[]>([]);
	let notes = $state<Note[]>([]);
	let selected = $state('');
	let error = $state<string | null>(null);
	let model = $state<ModelStatus>({ state: 'absent' });
	let tags = $state<[string, number][]>([]);
	let query = $state('');
	let results = $state<Note[]>([]);

	const searching = $derived(query.trim() !== '');
	const activeTags = $derived(
		query
			.split(/\s+/)
			.filter((token) => token.startsWith('#'))
			.map((token) => token.slice(1).toLowerCase())
	);

	// Only the latest query's answer is kept, so a slow reply to an earlier
	// keystroke cannot overwrite a newer one.
	let searchRun = 0;
	async function runSearch() {
		const run = ++searchRun;
		if (!searching) {
			results = [];
			return;
		}
		try {
			const found = await search(query);
			if (run === searchRun) results = found;
		} catch (e) {
			error = String(e);
		}
	}

	$effect(() => {
		void query;
		void runSearch();
	});

	async function refresh() {
		try {
			[days, notes, tags] = await Promise.all([listDays(), getDay(selected), listTags()]);
			error = null;
		} catch (e) {
			error = String(e);
		}
		await runSearch();
	}

	async function select(date: string) {
		selected = date;
		query = '';
		await refresh();
	}

	/** Adds the tag to the search, or takes it out again if it is already there. */
	function toggleTag(tag: string) {
		const tokens = query.split(/\s+/).filter(Boolean);
		const kept = tokens.filter((token) => token.toLowerCase() !== `#${tag}`);
		query = (kept.length === tokens.length ? [...tokens, `#${tag}`] : kept).join(' ');
	}

	async function remove(note: Note) {
		try {
			await deleteNote(note.date, note.id);
			await refresh();
		} catch (e) {
			error = String(e);
		}
	}

	onMount(() => {
		const off: Promise<() => void>[] = [];
		void (async () => {
			selected = await today();
			await refresh();
			// A note saved from the capture window lands in another webview.
			off.push(onNoteUpdated(() => void refresh()));
			// The watcher fires this when a daily file is edited outside the app.
			off.push(onIndexRebuilt(() => void refresh()));
			// Enrichment finishing rewrites the note, so the card has to reload.
			off.push(onNoteEnriched(() => void refresh()));

			model = await modelStatus();
			off.push(onModelStatus((status) => (model = status)));
		})();
		return () => off.forEach((p) => void p.then((stop) => stop()));
	});

	const heading = $derived(
		selected
			? new Date(`${selected}T00:00:00`).toLocaleDateString(undefined, {
					weekday: 'long',
					day: 'numeric',
					month: 'long',
					year: 'numeric'
				})
			: ''
	);
</script>

<div class="flex h-screen bg-neutral-950 text-neutral-100">
	<aside class="w-56 shrink-0 overflow-y-auto border-r border-neutral-800 p-3">
		<h1 class="mb-4 px-2 text-sm font-semibold">Scratchnote</h1>
		<div class="flex flex-col gap-6">
			<DayList {days} selected={searching ? '' : selected} onselect={select} />
			<TagList {tags} active={activeTags} onselect={toggleTag} />
		</div>
	</aside>

	<main class="flex-1 overflow-y-auto p-6">
		<div class="mb-4">
			<SearchBar bind:value={query} />
		</div>

		<h2 class="mb-4 text-lg font-semibold">
			{#if searching}
				{results.length}
				{results.length === 1 ? 'result' : 'results'}
			{:else}
				{heading}
			{/if}
		</h2>

		{#if model.state === 'absent' || model.state === 'downloading'}
			<Onboarding status={model} />
		{/if}

		{#if error}
			<p class="rounded border border-red-900 bg-red-950 p-3 text-sm text-red-300">{error}</p>
		{/if}

		{#if searching}
			{#if results.length === 0}
				<p class="text-sm text-neutral-600">No notes match.</p>
			{:else}
				<ul class="flex flex-col gap-3">
					{#each results as note (note.id)}
						<li><NoteCard {note} ondelete={remove} ontag={toggleTag} showDate /></li>
					{/each}
				</ul>
			{/if}
		{:else if notes.length === 0}
			<p class="text-sm text-neutral-600">
				Nothing captured. Press Ctrl+Shift+Space to write a note.
			</p>
		{:else}
			<ul class="flex flex-col gap-3">
				{#each notes as note (note.id)}
					<li><NoteCard {note} ondelete={remove} ontag={toggleTag} /></li>
				{/each}
			</ul>
		{/if}
	</main>
</div>
