<script lang="ts">
	import { onMount, tick } from 'svelte';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		categoryNames,
		deleteNote,
		embeddingModelInfo,
		getDay,
		getSettings,
		listCategories,
		listDays,
		listSpaces,
		listTags,
		modelInfo,
		modelStatus,
		onEmbeddingStatus,
		onIndexRebuilt,
		onModelStatus,
		onNoteEnriched,
		onNoteUpdated,
		onOpenSettings,
		onSpacesChanged,
		retryEnrichment,
		saveNote,
		search,
		setTrayLabels,
		similarNotes,
		splitCategory,
		today,
		updateNote,
		updateNoteMeta,
		type DaySummary,
		type IndexEntry,
		type ModelStatus,
		type SpacesView,
		type NoteEdit,
		type Note
	} from '$lib/api';
	import AppHeader from '$lib/components/AppHeader.svelte';
	import ChatPanel from '$lib/components/ChatPanel.svelte';
	import CommandCenter from '$lib/components/CommandCenter.svelte';
	import DeleteNoteDialog from '$lib/components/DeleteNoteDialog.svelte';
	import ModelStatusBar from '$lib/components/ModelStatusBar.svelte';
	import NewNote from '$lib/components/NewNote.svelte';
	import NoteEditor from '$lib/components/NoteEditor.svelte';
	import NoteList from '$lib/components/NoteList.svelte';
	import Onboarding from '$lib/components/Onboarding.svelte';
	import Settings from '$lib/components/settings/Settings.svelte';
	import TagFilters from '$lib/components/TagFilters.svelte';
	import CalendarPage from '$lib/components/CalendarPage.svelte';
	import TagsPage from '$lib/components/TagsPage.svelte';
	import TimelineHeader from '$lib/components/TimelineHeader.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import { m } from '$lib/paraglide/messages';
	import { resumeStep } from '$lib/onboarding';
	import { queryTags, toggleTag } from '$lib/query';
	import type { Timeline } from '$lib/timeline';

	let days = $state<DaySummary[]>([]);
	/** The selected day's notes. */
	let notes = $state<Note[]>([]);
	let selected = $state('');
	/** The capture hotkey saves to today, so only today's empty list mentions it. */
	let todayDate = $state('');
	/** The empty day's editor is open, which takes the place of its message. */
	let writingEmpty = $state(false);
	let error = $state<string | null>(null);
	let model = $state<ModelStatus>({ state: 'absent' });
	let tags = $state<[string, number][]>([]);
	let categories = $state<[string, number][]>([]);
	/** Every category on the list, for the editor to offer. */
	let categoryList = $state<string[]>([]);
	let spaces = $state<SpacesView | null>(null);

	let timeline = $state<Timeline>({ kind: 'day' });
	/** Changes when a new page opens: another view, day or source note. A
	 *  search refined by its tag filters stays the same page. */
	const pageKey = $derived(
		timeline.kind === 'day'
			? `day:${selected}`
			: timeline.kind === 'similar'
				? `similar:${timeline.note.id}`
				: timeline.kind
	);
	/** The page's title has scrolled under the top bar, which shows it instead. */
	let titleCollapsed = $state(false);
	/** The notes a search matches, while the timeline lists them. */
	let results = $state<Note[]>([]);
	/** The notes close to a note, while the timeline lists them. */
	let similar = $state<Note[]>([]);
	const activeTags = $derived(timeline.kind === 'search' ? queryTags(timeline.query) : []);

	/** The note open in the editor. */
	let editing = $state<Note | null>(null);
	/** The note waiting on the delete confirmation. */
	let deleting = $state<Note | null>(null);
	/** The command center's query, kept between openings. */
	let query = $state('');
	let paletteOpen = $state(false);
	let settingsOpen = $state(false);
	let chatOpen = $state(false);
	const canChat = $derived(model.state === 'loaded' || model.state === 'idle');
	/** Similar notes come from the embedding model's vectors. */
	let embeddingInstalled = $state(false);
	const canSimilar = $derived(embeddingInstalled && model.state !== 'disabled');

	/** The page's title, in the page and, once it scrolls away, in the top bar. */
	const headerProps = $derived({
		timeline,
		selected,
		tagCount: tags.length,
		resultCount: results.length,
		onback: () => void select(selected),
		oncalendar: () => (timeline = { kind: 'calendar' })
	});

	async function refresh() {
		try {
			[days, notes, tags, categories, categoryList, spaces, todayDate] = await Promise.all([
				listDays(),
				getDay(selected),
				listTags(),
				listCategories(),
				categoryNames(),
				listSpaces(),
				today()
			]);
			error = null;
		} catch (e) {
			error = String(e);
		}
		await loadTimeline();
	}

	// Only the latest answer is kept, so a slow reply to an earlier query
	// cannot overwrite a newer one.
	let run = 0;
	/** Loads what the timeline lists in place of the day, if anything. */
	async function loadTimeline() {
		const mine = ++run;
		const current = timeline;
		if (current.kind !== 'search') results = [];
		if (current.kind !== 'similar') similar = [];
		try {
			if (current.kind === 'search') {
				const found = await search(current.query);
				if (mine === run) results = found;
			} else if (current.kind === 'similar') {
				const found = await similarNotes(current.note.id);
				if (mine === run) similar = found;
			}
		} catch (e) {
			error = String(e);
		}
	}

	async function select(date: string) {
		timeline = { kind: 'day' };
		selected = date;
		writingEmpty = false;
		await refresh();
	}

	async function showResults(q: string) {
		const trimmed = q.trim();
		timeline = trimmed ? { kind: 'search', query: trimmed } : { kind: 'day' };
		query = trimmed;
		await loadTimeline();
	}

	async function showSimilar(note: Note) {
		timeline = { kind: 'similar', note };
		await loadTimeline();
		document.querySelector('main')?.scrollTo({ top: 0 });
	}

	/** The note a chat citation led to, blinking while it is set. */
	let blinking = $state<string | null>(null);
	let blinkTimer: ReturnType<typeof setTimeout> | undefined;

	/** Open a note's day, bring the note into view and blink it. */
	async function openCited(entry: Pick<IndexEntry, 'id' | 'date'>) {
		await select(entry.date);
		// Cleared first so a second click on the same note blinks it again.
		blinking = null;
		await tick();
		blinking = entry.id;
		await tick();
		document
			.querySelector(`[data-note-id="${CSS.escape(entry.id)}"]`)
			?.scrollIntoView({ block: 'center', behavior: 'smooth' });
		clearTimeout(blinkTimer);
		// A little past the animation, which runs 1.2s.
		blinkTimer = setTimeout(() => (blinking = null), 1400);
	}

	/**
	 * A tag clicked on a card opens the command center filtered on it. While
	 * the timeline lists results, it narrows them instead, see `toggleTag`.
	 */
	function openTag(tag: string, additive = false) {
		if (timeline.kind !== 'search') {
			query = `#${tag} `;
			paletteOpen = true;
			return;
		}
		void showResults(toggleTag(timeline.query, tag, additive));
	}

	function onWindowKeydown(event: KeyboardEvent) {
		if (event.key === '/' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			paletteOpen = !paletteOpen;
		}
	}

	/** The inline editor: the body alone, which leaves the labels to the model. */
	async function saveBody(note: Note, body: string): Promise<boolean> {
		try {
			if (body.trim() !== note.body) await updateNote(note.date, note.id, body);
			error = null;
			await refresh();
			return true;
		} catch (e) {
			error = String(e);
			return false;
		}
	}

	/** The full editor. Resolves to an error for it to show, or null once saved. */
	async function saveEdit(note: Note, edit: NoteEdit): Promise<string | null> {
		const [category, rest] = splitCategory(categoryList, note.tags);
		const tags = edit.tags.map((tag) => tag.replace(/^#+/, '')).filter(Boolean);
		const bodyChanged = edit.body.trim() !== note.body;
		// Only a real change to subject, category or tags takes the note away
		// from the model, so an edit to the body alone leaves it to be
		// re-enriched.
		const metaChanged =
			edit.subject.trim() !== (note.subject ?? '') ||
			edit.category !== category ||
			tags.join(' ') !== rest.join(' ');
		try {
			if (bodyChanged) await updateNote(note.date, note.id, edit.body);
			if (metaChanged)
				await updateNoteMeta(note.date, note.id, {
					subject: edit.subject,
					category: edit.category,
					tags
				});
			error = null;
			await refresh();
			return null;
		} catch (e) {
			await refresh();
			return String(e);
		}
	}

	/** A note written in the page rather than the capture window. */
	/** Add a note to the day shown. */
	async function addNote(body: string): Promise<boolean> {
		try {
			await saveNote(body, selected);
			error = null;
			await select(selected);
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
			if (editing?.id === note.id) editing = null;
			if (timeline.kind === 'similar' && timeline.note.id === note.id) timeline = { kind: 'day' };
			await refresh();
		} catch (e) {
			error = String(e);
		}
	}

	/** What every card on the page can do. */
	const cardActions = $derived({
		onedit: (note: Note) => (editing = note),
		ondelete: (note: Note) => (deleting = note),
		onsave: saveBody,
		onretry: retry,
		ontag: openTag,
		onsimilar: canSimilar ? showSimilar : undefined
	});

	onMount(() => {
		const off: Promise<() => void>[] = [];
		void (async () => {
			// A fresh install goes through the onboarding first. Someone who
			// already has a model, or turned it off, is left alone. A run under
			// way, as after the restart a new folder takes, picks up again.
			const [settings, info] = await Promise.all([getSettings(), modelInfo()]);
			const fresh =
				!settings.onboarded &&
				settings.modelEnabled &&
				!info.activePath &&
				!info.light &&
				!info.default;
			if (fresh || resumeStep() !== null) {
				await goto(resolve('/onboarding/'));
				return;
			}

			selected = await today();
			await refresh();
			// A note saved from the capture window lands in another webview.
			off.push(onNoteUpdated(() => void refresh()));
			// The watcher fires this when a daily file is edited outside the app.
			off.push(onIndexRebuilt(() => void refresh()));
			// Enrichment finishing rewrites the note, so the card has to reload.
			off.push(onNoteEnriched(() => void refresh()));
			off.push(onOpenSettings(() => (settingsOpen = true)));
			// Another space has its own days and notes, so a search or similar
			// notes from the last one would mean nothing there. Its tags and
			// days are listed afresh.
			off.push(
				onSpacesChanged((view) => {
					const switched = view.active !== spaces?.active;
					spaces = view;
					if (switched) {
						query = '';
						if (timeline.kind === 'search' || timeline.kind === 'similar')
							timeline = { kind: 'day' };
					}
					void refresh();
				})
			);

			embeddingInstalled = (await embeddingModelInfo()).installed;
			off.push(onEmbeddingStatus((status) => (embeddingInstalled = status.state === 'installed')));
			model = await modelStatus();
			off.push(onModelStatus((status) => (model = status)));
		})();
		return () => off.forEach((p) => void p.then((stop) => stop()));
	});

	// The tray menu lives in Rust; the main window keeps it in the open language.
	$effect(() => {
		void setTrayLabels({
			newNote: m.tray_new_note(),
			open: m.tray_open(),
			settings: m.common_settings(),
			quit: m.tray_quit()
		}).catch((e) => (error = String(e)));
	});
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="flex h-screen flex-col bg-neutral-950 text-neutral-100">
	<AppHeader
		{spaces}
		onsearch={() => (paletteOpen = true)}
		onsettings={() => (settingsOpen = true)}
		titleShown={titleCollapsed}
	>
		{#snippet title()}
			<TimelineHeader compact {...headerProps} />
		{/snippet}
	</AppHeader>

	<main class="min-h-0 flex-1 overflow-y-auto px-6 pb-28">
		<div class="mx-auto max-w-3xl">
			<TimelineHeader {...headerProps} oncollapse={(collapsed) => (titleCollapsed = collapsed)} />

			{#if timeline.kind === 'search'}
				<TagFilters
					active={activeTags}
					{results}
					onremove={(tag) => openTag(tag, true)}
					onadd={(tag) => openTag(tag, true)}
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

			<!-- Keyed on the view, so each one rises into place as it opens. -->
			{#key pageKey}
				<div class="page-in">
					{#if timeline.kind === 'calendar'}
						<CalendarPage {days} {selected} onselect={(date) => void select(date)} />
					{:else if timeline.kind === 'tags'}
						<TagsPage {tags} {categories} onpick={(tag) => void showResults(`#${tag}`)} />
					{:else if timeline.kind === 'similar'}
						<p
							class="mb-6 line-clamp-3 rounded-lg border border-neutral-800 px-3 py-2 text-sm whitespace-pre-wrap text-neutral-400"
						>
							{timeline.note.body}
						</p>
						<NoteList notes={similar} empty={m.page_no_similar()} showDate {...cardActions} />
					{:else if timeline.kind === 'search'}
						<NoteList notes={results} empty={m.page_no_match()} showDate {...cardActions} />
					{:else}
						{#if notes.length === 0}
							<div class="flex flex-col items-center gap-4 py-16 text-center">
								{#if !writingEmpty}
									<p class="text-base text-neutral-600">
										{selected === todayDate
											? m.page_empty_day({ hotkey: 'Ctrl+Shift+Space' })
											: m.page_empty_other_day()}
									</p>
								{/if}
								<NewNote onsave={addNote} centered bind:writing={writingEmpty} />
							</div>
						{:else}
							<NoteList {notes} empty="" {blinking} {...cardActions} />
							<NewNote onsave={addNote} />
						{/if}
					{/if}
				</div>
			{/key}
		</div>
	</main>
</div>

<div class="fixed bottom-3 left-3 z-30">
	<ModelStatusBar status={model} />
</div>

<ChatPanel
	bind:open={chatOpen}
	space={spaces?.active ?? ''}
	{canChat}
	modelOff={model.state === 'disabled'}
	onopen={(entry) => void openCited(entry)}
/>

<CommandCenter
	bind:open={paletteOpen}
	bind:query
	{tags}
	{categories}
	{canChat}
	onpick={(note) => void openCited(note)}
	onseeall={(q) => void showResults(q)}
	ontoday={async () => void select(await today())}
	ontags={() => (timeline = { kind: 'tags' })}
	onchat={() => (chatOpen = true)}
	onsettings={() => (settingsOpen = true)}
/>

<NoteEditor
	bind:note={editing}
	categories={categoryList}
	{tags}
	onsave={saveEdit}
	ondelete={(n) => (deleting = n)}
/>

<DeleteNoteDialog bind:note={deleting} onconfirm={remove} />

<Dialog.Root bind:open={settingsOpen}>
	<Dialog.Content
		class="h-[min(640px,85vh)] grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-[min(48rem,calc(100%-2rem))]"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>
