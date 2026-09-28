<script lang="ts">
	import { onMount, tick, untrack } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import {
		categoryNames,
		deleteNote,
		deletePage,
		isPage,
		noteToPage,
		onNewPage,
		embeddingModelInfo,
		getDay,
		getSettings,
		listCategories,
		listDays,
		listSpaces,
		modelInfo,
		modelStatus,
		onEmbeddingStatus,
		onIndexRebuilt,
		onModelStatus,
		onNoteEnriched,
		onNoteUpdated,
		onOpenSettings,
		onSettingsChanged,
		onSpacesChanged,
		retryEnrichment,
		saveNote,
		search,
		setSettings,
		setTrayLabels,
		similarNotes,
		today,
		updateNote,
		updateNoteMeta,
		type DaySummary,
		type IndexEntry,
		type ModelStatus,
		type SettingsView,
		type SpacesView,
		type NoteEdit,
		type Note
	} from '$lib/api';
	import AppHeader from '$lib/components/AppHeader.svelte';
	import ChatPanel from '$lib/components/ChatPanel.svelte';
	import CommandCenter from '$lib/components/CommandCenter.svelte';
	import DeleteNoteDialog from '$lib/components/DeleteNoteDialog.svelte';
	import Markdown from '$lib/components/Markdown.svelte';
	import ModelStatusBar from '$lib/components/ModelStatusBar.svelte';
	import NewNote from '$lib/components/NewNote.svelte';
	import NoteEditor from '$lib/components/NoteEditor.svelte';
	import NoteList from '$lib/components/NoteList.svelte';
	import NoteToPageDialog from '$lib/components/NoteToPageDialog.svelte';
	import PageView from '$lib/components/PageView.svelte';
	import Onboarding from '$lib/components/Onboarding.svelte';
	import Settings from '$lib/components/settings/Settings.svelte';
	import CalendarPage from '$lib/components/CalendarPage.svelte';
	import TimelineHeader from '$lib/components/TimelineHeader.svelte';
	import WhatsNew from '$lib/components/WhatsNew.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import PanelRightCloseIcon from '@lucide/svelte/icons/panel-right-close';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import { resumeStep } from '$lib/onboarding';
	import { categoryLabel } from '$lib/categories';
	import { compareVersions, FIRST_RELEASE, releasesSince, type Release } from '$lib/changelog';
	import { toggleCategory } from '$lib/query';
	import { addToPageDraft } from '$lib/page-draft';
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
	let categories = $state<[string, number][]>([]);
	/** Every category on the list, for the editor to offer. */
	let categoryList = $state<string[]>([]);
	let spaces = $state<SpacesView | null>(null);

	let timeline = $state<Timeline>({ kind: 'day' });
	/** Counts the pages opened, so opening one is a new view while a new
	 *  page getting its id on its first save is not. */
	let pageSession = $state(0);
	/** Changes when a new page opens: another view, day or source note. A
	 *  search refined by a category click stays the same page. */
	const pageKey = $derived(
		timeline.kind === 'day'
			? `day:${selected}`
			: timeline.kind === 'similar'
				? `similar:${timeline.note.id}`
				: timeline.kind === 'page'
					? `page:${pageSession}`
					: timeline.kind
	);
	/** The page's title has scrolled under the top bar, which shows it instead. */
	let titleCollapsed = $state(false);
	/** The notes a search matches, while the timeline lists them. */
	let results = $state<Note[]>([]);
	/** The notes close to a note, while the timeline lists them. */
	let similar = $state<Note[]>([]);

	/** The note open in the editor. */
	let editing = $state<Note | null>(null);
	/** The note waiting on the delete confirmation. */
	let deleting = $state<Note | null>(null);
	/** The note waiting on a title to become a page. */
	let turning = $state<Note | null>(null);
	/** The page docked on the right, to write in while the day stays in reach. */
	let docked = $state<Note | null>(null);
	/** The command center's query, kept between openings. */
	let query = $state('');
	let paletteOpen = $state(false);
	let settingsOpen = $state(false);
	/** Release notes waiting to be read after an update. */
	let releaseNotes = $state<Release[] | null>(null);
	let chatOpen = $state(false);
	const canChat = $derived(model.state === 'loaded' || model.state === 'idle');
	/** Similar notes come from the embedding model's vectors. */
	let embeddingInstalled = $state(false);
	const canSimilar = $derived(embeddingInstalled && model.state !== 'disabled');

	/** The days the arrows step through: those with notes, and today. Dates
	 *  sort as strings. */
	const stops = $derived(
		[...new Set([...days.map((day) => day.date), todayDate])].filter(Boolean).sort()
	);
	const previousDay = $derived(stops.findLast((date) => date < selected));
	const nextDay = $derived(stops.find((date) => date > selected));

	/** The timeline's width, which a docked page narrows. Taken with its
	 *  scrollbar, which comes and goes with what the columns hold. */
	let timelineWidth = $state(0);
	/** The text size setting, in pixels: the rem the columns are sized in. */
	let textSize = $state(16);
	/** How many days fit side by side in the timeline: the day alone, then
	 *  with the day before it from Tailwind's lg (64rem), then between both
	 *  neighbours from 2xl (96rem). The neighbours are the days the arrows
	 *  step to. */
	const fit = $derived.by(() => {
		const rems = timelineWidth / textSize;
		return rems >= 96 ? 3 : rems >= 64 ? 2 : 1;
	});
	/** Only the day view spreads over columns. */
	const columns = $derived(timeline.kind === 'day' ? fit : 1);
	/** The neighbours' notes, by date. Only the ones that fit are read. */
	let beside = $state<Record<string, Note[]>>({});

	/** The page's title, in the page and, once it scrolls away, in the top bar. */
	const headerProps = $derived({
		timeline,
		selected,
		resultCount: results.length,
		onback: () => void select(selected),
		oncalendar: () => (timeline = { kind: 'calendar' }),
		onprevious: previousDay ? () => void step(previousDay) : undefined,
		onnext: nextDay ? () => void step(nextDay) : undefined
	});

	async function refresh() {
		// Stepping through days asks for them in quick succession. An answer
		// for a day no longer shown is dropped, so a slow one cannot land last;
		// the refresh for the day shown now is on its way.
		const date = selected;
		try {
			const loaded = await Promise.all([
				listDays(),
				getDay(date),
				listCategories(),
				categoryNames(),
				listSpaces(),
				today()
			]);
			if (date !== selected) return;
			[days, notes, categories, categoryList, spaces, todayDate] = loaded;
			error = null;
		} catch (e) {
			if (date !== selected) return;
			error = String(e);
		}
		await Promise.all([loadTimeline(), loadBeside()]);
	}

	let besideRun = 0;
	/** Loads the neighbours shown beside the day, and lets go of any no longer shown. */
	async function loadBeside() {
		const mine = ++besideRun;
		const dates = (fit === 3 ? [previousDay, nextDay] : fit === 2 ? [previousDay] : []).filter(
			(date) => date !== undefined
		);
		try {
			const loaded = await Promise.all(dates.map((date) => getDay(date)));
			if (mine === besideRun)
				beside = Object.fromEntries(dates.map((date, i) => [date, loaded[i]]));
		} catch (e) {
			if (mine === besideRun) error = String(e);
		}
	}

	// Widening the timeline brings the neighbours in; narrowing it, with the
	// window or a docked page, lets them go.
	$effect(() => {
		void fit;
		untrack(() => void loadBeside());
	});

	/** A neighbour's date, over its column. */
	const besideHeading = (date: string) =>
		new Date(`${date}T00:00:00`).toLocaleDateString(getLocale(), {
			weekday: 'long',
			day: 'numeric',
			month: 'long',
			year: 'numeric'
		});

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

	/** Open a neighbouring day at its top, as the arrows in the top bar would leave it scrolled. */
	async function step(date: string) {
		await select(date);
		document.querySelector('main')?.scrollTo({ top: 0 });
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

	/** Open a page in the timeline's place, with its day to go back to. A
	 *  docked page leaves the dock, so no page is open in two editors at once. */
	async function openPage(page: Pick<Note, 'id' | 'date'>) {
		if (docked?.id === page.id) docked = null;
		selected = page.date;
		timeline = { kind: 'page', id: page.id };
		pageSession++;
		await tick();
		document.querySelector('main')?.scrollTo({ top: 0 });
	}

	/** Start a page on the day shown. Its draft comes back if there is one. */
	function newPage() {
		timeline = { kind: 'page', id: null };
		pageSession++;
	}

	/** The capture window handed over its draft. An open draft is closed
	 *  first, so it keeps what was typed in it and the text joins that. */
	async function takeCaptureDraft(body: string) {
		if (timeline.kind === 'page' && timeline.id === null) {
			timeline = { kind: 'day' };
			await tick();
		}
		addToPageDraft(body);
		if (!selected) selected = await today();
		newPage();
	}

	/** A new page has its title and its file; the view stays as it is. */
	function pageCreated(page: Note) {
		timeline = { kind: 'page', id: page.id };
		void refresh();
	}

	/** Resolves to an error for the dialog to show, or null once done. */
	async function turnIntoPage(note: Note, title: string): Promise<string | null> {
		try {
			const page = await noteToPage(note.date, note.id, title);
			error = null;
			await refresh();
			await openPage(page);
			return null;
		} catch (e) {
			return String(e);
		}
	}

	/** Open a note's day, bring the note into view and blink it. A page opens. */
	async function openCited(entry: Pick<IndexEntry, 'id' | 'date' | 'kind'>) {
		if (isPage(entry)) {
			await openPage(entry);
			return;
		}
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
	 * A category clicked on a card opens the command center filtered on it.
	 * While the timeline lists results, it narrows them instead, see
	 * `toggleCategory`.
	 */
	function openCategory(category: string) {
		if (timeline.kind !== 'search') {
			query = `#${categoryLabel(category)} `;
			paletteOpen = true;
			return;
		}
		void showResults(toggleCategory(timeline.query, categoryLabel(category)));
	}

	function onWindowKeydown(event: KeyboardEvent) {
		if (event.key === '/' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			paletteOpen = !paletteOpen;
		} else if (
			event.altKey &&
			(event.key === 'ArrowLeft' || event.key === 'ArrowRight') &&
			timeline.kind === 'day'
		) {
			// Text keeps its Alt+arrows, which move by word on macOS, and a
			// dialog keeps them from the day behind it.
			const target = event.target as HTMLElement;
			if (target.isContentEditable || target.closest('input, textarea, [role="dialog"]')) return;
			// Also stops the webview going back in its history.
			event.preventDefault();
			const date = event.key === 'ArrowLeft' ? previousDay : nextDay;
			if (date) void step(date);
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
		const bodyChanged = edit.body.trim() !== note.body;
		// Only a real change to subject or category takes the note away from
		// the model, so an edit to the body alone leaves it to be re-enriched.
		const metaChanged =
			edit.subject.trim() !== (note.subject ?? '') || edit.category !== (note.category ?? '');
		try {
			if (bodyChanged) await updateNote(note.date, note.id, edit.body);
			if (metaChanged)
				await updateNoteMeta(note.date, note.id, {
					subject: edit.subject,
					category: edit.category
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
			if (isPage(note)) await deletePage(note.date, note.id);
			else await deleteNote(note.date, note.id);
			if (editing?.id === note.id) editing = null;
			if (timeline.kind === 'similar' && timeline.note.id === note.id) timeline = { kind: 'day' };
			if (timeline.kind === 'page' && timeline.id === note.id) timeline = { kind: 'day' };
			if (docked?.id === note.id) docked = null;
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
		oncategory: openCategory,
		onsimilar: canSimilar ? showSimilar : undefined,
		onopen: (page: Note) => void openPage(page),
		ondock: (page: Note) => (docked = page),
		onpage: (note: Note) => (turning = note),
		onerror: showError
	});

	function showError(message: string) {
		error = message;
	}

	/**
	 * After an update, the notes of every release since the version last
	 * opened. A settings file without one is from 0.1.0, which kept none.
	 */
	async function showReleaseNotes(settings: SettingsView) {
		const current = await getVersion();
		const since = settings.lastSeenVersion ?? FIRST_RELEASE;
		if (compareVersions(current, since) <= 0) return;
		const news = releasesSince(since, current);
		if (news.length) releaseNotes = news;
		await setSettings({ ...settings, lastSeenVersion: current });
	}

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
			void showReleaseNotes(settings).catch((e) => showError(String(e)));

			textSize = settings.fontSize;
			off.push(onSettingsChanged((changed) => (textSize = changed.fontSize)));
			selected = await today();
			await refresh();
			// A note saved from the capture window lands in another webview.
			off.push(onNoteUpdated(() => void refresh()));
			// The watcher fires this when a daily file is edited outside the app.
			off.push(onIndexRebuilt(() => void refresh()));
			// Enrichment finishing rewrites the note, so the card has to reload.
			off.push(onNoteEnriched(() => void refresh()));
			off.push(onOpenSettings(() => (settingsOpen = true)));
			off.push(onNewPage((body) => void takeCaptureDraft(body)));
			// Another space has its own days and notes, so a search, similar
			// notes or a page from the last one would mean nothing there. Its
			// categories and days are listed afresh.
			off.push(
				onSpacesChanged((view) => {
					const switched = view.active !== spaces?.active;
					spaces = view;
					if (switched) {
						query = '';
						docked = null;
						if (
							timeline.kind === 'search' ||
							timeline.kind === 'similar' ||
							timeline.kind === 'page'
						)
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

<div
	class="flex h-screen flex-col bg-neutral-950 text-neutral-100"
	data-model-off={model.state === 'disabled' || undefined}
>
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

	<div class="flex min-h-0 flex-1">
		<!-- In a wider timeline the day has the one before it on its left, and in
		     a wide one the one after it on its right too, which centres it. -->
		<main
			bind:offsetWidth={timelineWidth}
			class={[
				'min-w-0 flex-1 overflow-y-auto px-6 pb-16',
				columns > 1 && 'grid justify-center gap-x-8',
				columns === 2 && 'grid-cols-[repeat(2,minmax(0,48rem))]',
				columns === 3 && 'grid-cols-[repeat(3,minmax(0,48rem))]'
			]}
		>
			{#if columns > 1}{@render besideDay(previousDay)}{/if}
			<div class="mx-auto w-full max-w-3xl">
				<TimelineHeader {...headerProps} oncollapse={(collapsed) => (titleCollapsed = collapsed)} />

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
						{:else if timeline.kind === 'page'}
							<PageView
								id={timeline.id}
								date={selected}
								oncreated={pageCreated}
								ondelete={(page) => (deleting = page)}
								onretry={retry}
								oncategory={openCategory}
								onsimilar={canSimilar ? showSimilar : undefined}
							/>
						{:else if timeline.kind === 'similar'}
							<div
								class="mb-6 rounded-lg border border-neutral-800 px-3 py-2 text-sm text-neutral-400"
							>
								<Markdown
									text={timeline.note.body}
									links={false}
									class="max-h-[3lh] overflow-hidden"
								/>
							</div>
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
									<NewNote
										onsave={addNote}
										onpage={newPage}
										onerror={showError}
										centered
										bind:writing={writingEmpty}
									/>
								</div>
							{:else}
								<NoteList {notes} empty="" {blinking} {...cardActions} />
								<NewNote onsave={addNote} onpage={newPage} onerror={showError} />
							{/if}
						{/if}
					</div>
				{/key}
			</div>
			{#if columns === 3}{@render besideDay(nextDay)}{/if}
		</main>
		{#if docked}
			{@const page = docked}
			<!-- The page docked beside the day, written in while the notes stay
			     in reach. -->
			<aside
				aria-label={page.subject ?? m.pages_untitled()}
				class="flex w-(--page-dock) shrink-0 flex-col border-l border-neutral-800"
			>
				<div class="flex justify-end px-3 pt-3">
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={() => (docked = null)}
						aria-label={m.common_close()}
						title={m.common_close()}
						class="text-muted-foreground hover:text-foreground"
					>
						<PanelRightCloseIcon />
					</Button>
				</div>
				<div class="min-h-0 flex-1 overflow-y-auto px-8">
					{#key page.id}
						<PageView
							id={page.id}
							date={page.date}
							oncreated={() => void refresh()}
							ondelete={(p) => (deleting = p)}
							onretry={retry}
							oncategory={openCategory}
							onsimilar={canSimilar ? showSimilar : undefined}
						/>
					{/key}
				</div>
			</aside>
		{/if}
	</div>
</div>

<!-- A neighbour's column. Its date makes it the selected day; it has no new
     note of its own. Left empty at either end, so the day keeps its place. -->
{#snippet besideDay(date: string | undefined)}
	<section>
		{#if date}
			<button
				type="button"
				onclick={() => void step(date)}
				class="mt-6 mb-8 block max-w-full cursor-pointer truncate text-lg leading-8 font-medium text-muted-foreground transition-colors hover:text-foreground"
			>
				{besideHeading(date)}
			</button>
			{#key date}
				<div class="page-in">
					{#if beside[date]}
						<NoteList notes={beside[date]} empty={m.page_empty_other_day()} {...cardActions} />
					{/if}
				</div>
			{/key}
		{/if}
	</section>
{/snippet}

<div class="fixed bottom-3 left-3 z-30">
	<ModelStatusBar status={model} />
</div>

<ChatPanel
	bind:open={chatOpen}
	space={spaces?.active ?? ''}
	{canChat}
	modelOff={model.state === 'disabled'}
	docked={docked !== null}
	onopen={(entry) => void openCited(entry)}
/>

<CommandCenter
	bind:open={paletteOpen}
	bind:query
	{categories}
	{canChat}
	canMeaning={canSimilar}
	onpick={(note) => void openCited(note)}
	onseeall={(q) => void showResults(q)}
	ontoday={async () => void select(await today())}
	onnewpage={newPage}
	onchat={() => (chatOpen = true)}
	onsettings={() => (settingsOpen = true)}
/>

<NoteEditor
	bind:note={editing}
	categories={categoryList}
	onsave={saveEdit}
	ondelete={(n) => (deleting = n)}
/>

<DeleteNoteDialog bind:note={deleting} onconfirm={remove} />

<NoteToPageDialog bind:note={turning} onconfirm={turnIntoPage} />

<WhatsNew bind:releases={releaseNotes} />

<Dialog.Root bind:open={settingsOpen}>
	<Dialog.Content
		class="h-[min(640px,85vh)] grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-[min(48rem,calc(100%-2rem))]"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>
