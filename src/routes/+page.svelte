<script lang="ts">
	import { onMount } from 'svelte';
	import {
		deleteNote,
		enrichBusy,
		enrichProgress,
		getDay,
		listCategories,
		listDays,
		listTags,
		modelStatus,
		onEnrichBusy,
		onEnrichProgress,
		onIndexRebuilt,
		onModelStatus,
		onNoteEnriched,
		onNoteUpdated,
		onOpenSettings,
		retryEnrichment,
		search,
		today,
		updateNote,
		updateNoteMeta,
		type DaySummary,
		type EnrichProgress,
		type ModelStatus,
		type NoteEdit,
		type Note
	} from '$lib/api';
	import DayCalendar from '$lib/components/DayCalendar.svelte';
	import ModelStatusBar from '$lib/components/ModelStatusBar.svelte';
	import NoteCard from '$lib/components/NoteCard.svelte';
	import Onboarding from '$lib/components/Onboarding.svelte';
	import SearchBar from '$lib/components/SearchBar.svelte';
	import Settings from '$lib/components/Settings.svelte';
	import TagFilters from '$lib/components/TagFilters.svelte';
	import CategoryList from '$lib/components/CategoryList.svelte';
	import WindowControls from '$lib/components/WindowControls.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import SettingsIcon from '@lucide/svelte/icons/settings';

	let days = $state<DaySummary[]>([]);
	let notes = $state<Note[]>([]);
	let selected = $state('');
	let error = $state<string | null>(null);
	let model = $state<ModelStatus>({ state: 'absent' });
	let busy = $state(false);
	let progress = $state<EnrichProgress | null>(null);
	let tags = $state<[string, number][]>([]);
	let categories = $state<[string, number][]>([]);
	let query = $state('');
	let results = $state<Note[]>([]);
	let settingsOpen = $state(false);

	// macOS keeps its native traffic lights over the sidebar; elsewhere the
	// window is undecorated and draws its own controls.
	const mac = navigator.userAgent.includes('Mac');

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
			[days, notes, tags, categories] = await Promise.all([
				listDays(),
				getDay(selected),
				listTags(),
				listCategories()
			]);
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

	/**
	 * A plain click filters on this tag alone, or clears it if it already was
	 * the only one. With `additive`, the tag is added to the search or taken
	 * out again. Words that are not tags are kept either way.
	 */
	function toggleTag(tag: string, additive = false) {
		const tokens = query.split(/\s+/).filter(Boolean);
		const kept = tokens.filter((token) => token.toLowerCase() !== `#${tag}`);
		const selected = kept.length !== tokens.length;
		if (additive) {
			query = (selected ? kept : [...tokens, `#${tag}`]).join(' ');
			return;
		}
		const words = tokens.filter((token) => !token.startsWith('#'));
		const alone = selected && activeTags.length === 1;
		query = (alone ? words : [...words, `#${tag}`]).join(' ');
	}

	async function save(note: Note, edit: NoteEdit): Promise<boolean> {
		const tags = edit.tags.map((tag) => tag.replace(/^#+/, '')).filter(Boolean);
		const bodyChanged = edit.body.trim() !== note.body;
		// Only a real change to subject or tags takes the note away from the
		// model, so an edit to the body alone leaves it to be re-enriched.
		const metaChanged =
			edit.subject.trim() !== (note.subject ?? '') || tags.join(' ') !== note.tags.join(' ');
		try {
			if (bodyChanged) await updateNote(note.date, note.id, edit.body);
			if (metaChanged) await updateNoteMeta(note.date, note.id, { subject: edit.subject, tags });
			error = null;
			await refresh();
			return true;
		} catch (e) {
			error = String(e);
			return false;
		}
	}

	async function retry(note: Note) {
		try {
			await retryEnrichment(note.date, note.id);
		} catch (e) {
			error = String(e);
		}
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
			off.push(onOpenSettings(() => (settingsOpen = true)));

			model = await modelStatus();
			off.push(onModelStatus((status) => (model = status)));
			busy = await enrichBusy();
			off.push(
				onEnrichBusy((value) => {
					busy = value;
					if (!value) progress = null;
				})
			);
			progress = await enrichProgress();
			off.push(onEnrichProgress((value) => (progress = value)));
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
	<aside class="flex w-56 shrink-0 flex-col gap-4 border-r bg-muted/40 p-3">
		<div
			data-tauri-drag-region="deep"
			class={['flex items-center justify-between', mac ? 'pl-16' : 'pl-2']}
		>
			<h1 class="text-base leading-none font-medium">Scratchnote</h1>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={() => (settingsOpen = true)}
				aria-label="Settings"
				title="Settings"
				class="text-muted-foreground hover:text-foreground"
			>
				<SettingsIcon />
			</Button>
		</div>
		<div class="flex min-h-0 flex-1 flex-col gap-6">
			<DayCalendar {days} selected={searching ? '' : selected} onselect={select} />
			<CategoryList {categories} active={activeTags} onselect={toggleTag} />
		</div>
		<ModelStatusBar status={model} {busy} {progress} />
	</aside>

	<main class="flex min-w-0 flex-1 flex-col">
		<div data-tauri-drag-region class="flex h-10 shrink-0 items-center justify-end px-2">
			{#if !mac}<WindowControls />{/if}
		</div>
		<div class="flex-1 overflow-y-auto px-6 pb-6">
			<div class="mx-auto max-w-2xl">
				<div class="mb-4">
					<SearchBar bind:value={query} {tags} />
				</div>

				<h2 class="mb-4 text-lg font-semibold">
					{#if searching}
						{results.length}
						{results.length === 1 ? 'result' : 'results'}
					{:else}
						{heading}
					{/if}
				</h2>

				{#if searching}
					<TagFilters
						active={activeTags}
						{results}
						onremove={(tag) => toggleTag(tag, true)}
						onadd={(tag) => toggleTag(tag, true)}
					/>
				{/if}

				{#if model.state === 'absent' || model.state === 'downloading'}
					<Onboarding status={model} />
				{/if}

				{#if error}
					<p
						class="rounded border border-red-200 bg-red-50 p-3 text-sm text-red-700 dark:border-red-900 dark:bg-red-950 dark:text-red-300"
					>
						{error}
					</p>
				{/if}

				{#if searching}
					{#if results.length === 0}
						<p class="text-sm text-neutral-600">No notes match.</p>
					{:else}
						<ul>
							{#each results as note (note.id)}
								<li>
									<NoteCard
										{note}
										ondelete={remove}
										onsave={save}
										onretry={retry}
										ontag={toggleTag}
										showDate
									/>
								</li>
							{/each}
						</ul>
					{/if}
				{:else if notes.length === 0}
					<p class="text-sm text-neutral-600">
						Nothing captured. Press Ctrl+Shift+Space to write a note.
					</p>
				{:else}
					<ul>
						{#each notes as note (note.id)}
							<li>
								<NoteCard
									{note}
									ondelete={remove}
									onsave={save}
									onretry={retry}
									ontag={toggleTag}
								/>
							</li>
						{/each}
					</ul>
				{/if}
			</div>
		</div>
	</main>
</div>

<Dialog.Root bind:open={settingsOpen}>
	<Dialog.Content
		class="h-[min(640px,85vh)] grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-3xl"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>
