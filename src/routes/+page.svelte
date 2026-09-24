<script lang="ts">
	import { onMount, tick } from 'svelte';
	import {
		categoryNames,
		deleteNote,
		enrichBusy,
		enrichProgress,
		getDay,
		listCategories,
		listDays,
		listSpaces,
		listTags,
		modelStatus,
		onEnrichBusy,
		onEnrichProgress,
		onIndexRebuilt,
		onModelStatus,
		onNoteEnriched,
		onNoteUpdated,
		onOpenSettings,
		onSpacesChanged,
		retryEnrichment,
		setTrayLabels,
		splitCategory,
		today,
		updateNote,
		updateNoteMeta,
		type DaySummary,
		type EnrichProgress,
		type IndexEntry,
		type ModelStatus,
		type SpacesView,
		type NoteEdit,
		type Note
	} from '$lib/api';
	import Chat from '$lib/components/Chat.svelte';
	import CommandCenter from '$lib/components/CommandCenter.svelte';
	import DayCalendar from '$lib/components/DayCalendar.svelte';
	import ModelStatusBar from '$lib/components/ModelStatusBar.svelte';
	import NoteCard from '$lib/components/NoteCard.svelte';
	import NoteEditor from '$lib/components/NoteEditor.svelte';
	import Onboarding from '$lib/components/Onboarding.svelte';
	import Settings from '$lib/components/Settings.svelte';
	import SpaceSwitcher from '$lib/components/SpaceSwitcher.svelte';
	import WindowControls from '$lib/components/WindowControls.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import SearchIcon from '@lucide/svelte/icons/search';
	import SettingsIcon from '@lucide/svelte/icons/settings';
	import SparklesIcon from '@lucide/svelte/icons/sparkles';
	import XIcon from '@lucide/svelte/icons/x';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';

	let days = $state<DaySummary[]>([]);
	let notes = $state<Note[]>([]);
	let selected = $state('');
	let error = $state<string | null>(null);
	let model = $state<ModelStatus>({ state: 'absent' });
	let busy = $state(false);
	let progress = $state<EnrichProgress | null>(null);
	let tags = $state<[string, number][]>([]);
	let categories = $state<[string, number][]>([]);
	/** Every category on the list, for the editor to offer. */
	let categoryList = $state<string[]>([]);
	/** The note open in the editor. */
	let editing = $state<Note | null>(null);
	/** The note waiting on the delete confirmation. */
	let deleting = $state<Note | null>(null);
	let removing = $state(false);
	/** The command center's query, kept between openings. */
	let query = $state('');
	let paletteOpen = $state(false);
	let settingsOpen = $state(false);
	let spaces = $state<SpacesView | null>(null);

	// macOS keeps its native traffic lights over the top left corner;
	// elsewhere the window is undecorated and draws its own controls.
	const mac = navigator.userAgent.includes('Mac');

	/** The chat about the open space's index, floating over the day. */
	let chatOpen = $state(false);
	const canChat = $derived(model.state === 'loaded' || model.state === 'idle');

	async function refresh() {
		try {
			[days, notes, tags, categories, categoryList, spaces] = await Promise.all([
				listDays(),
				getDay(selected),
				listTags(),
				listCategories(),
				categoryNames(),
				listSpaces()
			]);
			error = null;
		} catch (e) {
			error = String(e);
		}
	}

	async function select(date: string) {
		selected = date;
		await refresh();
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

	/** A tag clicked on a card opens the command center filtered on it. */
	function openTag(tag: string) {
		query = `#${tag} `;
		paletteOpen = true;
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

	async function retry(note: Note) {
		try {
			await retryEnrichment(note.date, note.id);
		} catch (e) {
			error = String(e);
		}
	}

	async function remove() {
		const note = deleting;
		if (!note || removing) return;
		removing = true;
		try {
			await deleteNote(note.date, note.id);
			if (editing?.id === note.id) editing = null;
			deleting = null;
			await refresh();
		} catch (e) {
			error = String(e);
			deleting = null;
		} finally {
			removing = false;
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
			// Another space has its own days and tags, so a search or tag
			// filter from the last one would mean nothing there.
			off.push(
				onSpacesChanged((view) => {
					const switched = view.active !== spaces?.active;
					spaces = view;
					if (switched) query = '';
					void refresh();
				})
			);

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

	// The tray menu lives in Rust; the main window keeps it in the open language.
	$effect(() => {
		void setTrayLabels({
			newNote: m.tray_new_note(),
			open: m.tray_open(),
			settings: m.common_settings(),
			quit: m.tray_quit()
		}).catch((e) => (error = String(e)));
	});

	const heading = $derived(
		selected
			? new Date(`${selected}T00:00:00`).toLocaleDateString(getLocale(), {
					weekday: 'long',
					day: 'numeric',
					month: 'long',
					year: 'numeric'
				})
			: ''
	);
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div class="flex h-screen flex-col bg-neutral-950 text-neutral-100">
	<header
		data-tauri-drag-region="deep"
		class={['flex h-12 shrink-0 items-center gap-1 pr-2', mac ? 'pl-20' : 'pl-4']}
	>
		<SpaceSwitcher view={spaces} />
		<div class="ml-auto flex items-center gap-1">
			<Button
				variant="ghost"
				size="sm"
				onclick={() => (paletteOpen = true)}
				aria-label={m.search_label()}
				title={m.search_label()}
				class="text-muted-foreground hover:text-foreground"
			>
				<SearchIcon />
				<kbd class="font-mono text-xs">{mac ? '⌘' : 'Ctrl'} /</kbd>
			</Button>
			<Button
				variant="ghost"
				size="icon-sm"
				onclick={() => (settingsOpen = true)}
				aria-label={m.common_settings()}
				title={m.common_settings()}
				class="text-muted-foreground hover:text-foreground"
			>
				<SettingsIcon />
			</Button>
			{#if !mac}<WindowControls />{/if}
		</div>
	</header>

	<main class="min-h-0 flex-1 overflow-y-auto px-6 pb-28">
		<div class="mx-auto max-w-3xl">
			<div class="mt-6 mb-8 flex items-center gap-2">
				<DayCalendar {days} {selected} onselect={select} />
				<h1 class="text-3xl font-semibold tracking-tight">{heading}</h1>
			</div>

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

			{#if notes.length === 0}
				<p class="text-base text-neutral-600">
					{m.page_empty_day({ hotkey: 'Ctrl+Shift+Space' })}
				</p>
			{:else}
				<ul>
					{#each notes as note (note.id)}
						<li>
							<NoteCard
								{note}
								onedit={(n) => (editing = n)}
								ondelete={(n) => (deleting = n)}
								onsave={saveBody}
								onretry={retry}
								ontag={openTag}
								blink={note.id === blinking}
							/>
						</li>
					{/each}
				</ul>
			{/if}
		</div>
	</main>
</div>

<div class="fixed bottom-3 left-3 z-30">
	<ModelStatusBar status={model} {busy} {progress} />
</div>

{#if chatOpen}
	<section
		aria-label={m.chat_title()}
		class="fixed right-6 bottom-16 z-40 flex h-[min(40rem,calc(100vh-7rem))] w-[min(26rem,calc(100vw-3rem))] flex-col overflow-hidden rounded-xl border bg-popover text-popover-foreground shadow-2xl"
	>
		<!-- Another space has another index, so a switch starts a new chat. -->
		{#key spaces?.active}
			<Chat
				space={spaces?.active ?? ''}
				onclose={() => (chatOpen = false)}
				onopen={(entry) => void openCited(entry)}
			/>
		{/key}
	</section>
{/if}

<!-- A disabled button shows no title, so the wrapper carries it. -->
<span
	class="fixed right-6 bottom-2 z-40"
	title={chatOpen
		? m.common_close()
		: canChat
			? m.search_chat_title()
			: model.state === 'disabled'
				? m.search_chat_model_off()
				: m.search_chat_disabled()}
>
	<Button
		onclick={() => (chatOpen = !chatOpen)}
		disabled={!canChat && !chatOpen}
		aria-label={chatOpen ? m.common_close() : m.search_chat_label()}
		aria-expanded={chatOpen}
		class="size-12 rounded-full shadow-lg [&_svg:not([class*='size-'])]:size-5"
	>
		{#if chatOpen}<XIcon />{:else}<SparklesIcon />{/if}
	</Button>
</span>

<CommandCenter
	bind:open={paletteOpen}
	bind:query
	{tags}
	{categories}
	{canChat}
	onpick={(note) => void openCited(note)}
	ontoday={async () => void select(await today())}
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

<Dialog.Root
	open={deleting !== null}
	onOpenChange={(open) => {
		if (!open) deleting = null;
	}}
>
	<Dialog.Content showCloseButton={false}>
		<Dialog.Header>
			<Dialog.Title>{m.page_delete_title()}</Dialog.Title>
			<Dialog.Description>{m.page_delete_description()}</Dialog.Description>
		</Dialog.Header>
		{#if deleting}
			<p
				class="line-clamp-4 rounded-lg border bg-muted/40 px-3 py-2 text-sm whitespace-pre-wrap text-muted-foreground"
			>
				{deleting.body}
			</p>
		{/if}
		<Dialog.Footer>
			<Button variant="outline" onclick={() => (deleting = null)}>{m.common_cancel()}</Button>
			<Button variant="destructive" onclick={remove} disabled={removing}>
				{removing ? m.page_deleting() : m.common_delete()}
			</Button>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>

<Dialog.Root bind:open={settingsOpen}>
	<Dialog.Content
		class="h-[min(640px,85vh)] grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-[min(48rem,calc(100%-2rem))]"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>
