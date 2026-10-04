import { dev } from '$app/env';
import { invoke as tauriInvoke, type InvokeArgs, type InvokeOptions } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import type { Language } from '#lib/i18n.svelte.js';

/** A command the window called while it started, timed from when it began loading. */
export interface StartupCall {
	cmd: string;
	start: number;
	ms: number;
}

/**
 * In a dev build, the commands the window calls until its first view is up,
 * for the startup details (`#lib/startup.js`), which close the list then.
 */
export const startupCalls = { open: dev, calls: [] as StartupCall[] };

function invoke<T>(cmd: string, args?: InvokeArgs, options?: InvokeOptions): Promise<T> {
	if (!startupCalls.open) return tauriInvoke<T>(cmd, args, options);
	const start = performance.now();
	return tauriInvoke<T>(cmd, args, options).finally(() =>
		startupCalls.calls.push({ cmd, start, ms: performance.now() - start })
	);
}

/** What the app did at launch before its window loaded (dev builds show it). */
export interface Launch {
	/** When the process began, in ms since the Unix epoch. */
	began: number;
	steps: { step: string; ms: number }[];
}

/** The launch's steps, to the first window that asks; null after. */
export const launchSteps = () => invoke<Launch | null>('launch_steps');

export interface Note {
	id: string;
	date: string;
	time: string;
	file: string;
	/** A page's title; null on a note. */
	subject: string | null;
	hash: string;
	body: string;
	/** A page has a file of its own, and its title as its subject (SPEC 3.5). */
	kind?: 'page';
	/** A page whose stub is in the day's file but whose file is gone. */
	missing?: boolean;
	/** The later day it looks forward to, as read from its text (SPEC 5.3). */
	on?: string;
}

export const isPage = (note: Pick<Note, 'kind'>) => note.kind === 'page';

export interface DaySummary {
	date: string;
	count: number;
	/** How many words the day's notes hold, its pages left out. */
	words: number;
}

/**
 * Saves to today unless a date is given, and into the open space unless
 * another is named. Returns null when the body was empty, which is a no-op
 * rather than an error.
 */
export const saveNote = (body: string, date?: string, space?: string) =>
	invoke<Note | null>('save_note', { body, date, space });

export const getDay = (date: string) => invoke<Note[]>('get_day', { date });

export const listDays = () => invoke<DaySummary[]>('list_days');

/** The notes and pages that look forward to a day, oldest first (SPEC 5.3). */
export const notesAbout = (date: string) => invoke<Note[]>('notes_about', { date });

/** Forget the day ahead read in a note or page; `date` is its own day. */
export const clearDayAhead = (date: string, id: string) =>
	invoke<void>('clear_day_ahead', { date, id });

/** A stretch of the notes a search found, and how many it found in all. */
export interface Found {
	notes: Note[];
	total: number;
}

/**
 * Words must all appear. Newest first. The `limit` notes from `offset` on, or
 * every one without a limit.
 */
export const search = (query: string, limit?: number, offset = 0) =>
	invoke<Found>('search', { query, offset, limit });

/** Notes close in meaning that the words miss. Empty without the embedding model. */
export const searchMeaning = (query: string) => invoke<Note[]>('search_meaning', { query });

/**
 * The notes closest in meaning to this one, best first. Empty while the note
 * has no vector yet, or without the embedding model.
 */
export const similarNotes = (id: string) => invoke<Note[]>('similar_notes', { id });

/**
 * The old note a draft is about, when one stands out (SPEC 6.3). `exclude` is
 * the note the draft is an edit of, and `space` the one it goes into, when
 * the capture window picked one: nothing is found for a space not open. Null
 * without the embedding model.
 */
export const recall = (text: string, exclude?: string, space?: string) =>
	invoke<Note | null>('recall', { text, exclude, space });

/** The notes about one thing, gathered across days (SPEC 6.4). */
export interface Thread {
	/** The id of the note it started with, which it keeps. */
	id: string;
	/** The user's; null until they name it. */
	title: string | null;
	/** The title is the user's. */
	named: boolean;
	/** The user's, rather than only suggested: titled, or holding a note they put there. */
	kept: boolean;
	/** Its notes and pages, oldest first. */
	notes: string[];
	/** The day of its first note. */
	since: string;
	/** The day of its last note. */
	until: string;
}

/** A thread with its first two notes, to tell it from others with no title. */
export interface ThreadCard extends Thread {
	first: Note[];
}

export interface ThreadsView {
	threads: Thread[];
	/** The notes taken out of threads, which stay out. */
	alone: string[];
}

/** Every thread of the open space. Empty without the embedding model. */
export const listThreads = () => invoke<ThreadsView>('list_threads');

/** A thread with its notes, oldest first, or null once it is gone. */
export const getThread = (id: string) =>
	invoke<{ thread: Thread; notes: Note[] } | null>('get_thread', { id });

/** An empty title takes the user's name away. */
export const renameThread = (id: string, title: string) =>
	invoke<void>('rename_thread', { id, title });

/** Take a note out of threads, where it stays, or let it back in. */
export const keepOutOfThreads = (id: string, out: boolean) =>
	invoke<void>('keep_out_of_threads', { id, out });

/** Every thread with its first notes, the one written in last first. */
export const threadCards = () => invoke<ThreadCard[]>('thread_cards');

/** The threads a note could be put in, the one it fits best first. */
export const threadsForNote = (id: string) => invoke<ThreadCard[]>('threads_for_note', { id });

/** Make a suggested thread the user's, so notes like its own keep joining. */
export const keepThread = (id: string) => invoke<void>('keep_thread', { id });

/** Stop suggesting a thread until it holds another note. */
export const dismissThread = (id: string) => invoke<void>('dismiss_thread', { id });

/**
 * Put notes in a thread, or a new one when `into` is null, where they stay
 * whatever they score. Resolves to the thread's id.
 */
export const putInThread = (notes: string[], into: string | null) =>
	invoke<string>('put_in_thread', { notes, into });

/** Put every note of thread `from` in thread `into`. */
export const mergeThreads = (from: string, into: string) =>
	invoke<void>('merge_threads', { from, into });

/** Fired when the notes were placed in threads anew, or a thread renamed. */
export const onThreadsChanged = (handler: () => void): Promise<UnlistenFn> =>
	listen('threads-changed', () => handler());

/** A note's place on the map of the space by meaning (SPEC 6.5). */
export interface MapNote extends Pick<Note, 'id' | 'date' | 'kind'> {
	x: number;
	y: number;
}

/** Every note placed on the map. Empty until the embed task has placed them. */
export const noteMap = () => invoke<MapNote[]>('note_map');

/** One note or page with its text, or null once it is gone. */
export const getNote = (id: string) => invoke<Note | null>('get_note', { id });

/** Fired when notes moved on the map, or joined or left it. */
export const onMapChanged = (handler: () => void): Promise<UnlistenFn> =>
	listen('map-changed', () => handler());

/**
 * From the capture window: hide it, keeping its draft, and show the note in
 * the main window.
 */
export const revealNote = (note: Pick<Note, 'id' | 'date' | 'kind'>) =>
	invoke<void>('reveal_note', { id: note.id, date: note.date, kind: note.kind });

/** Fired in the main window when the capture window asks it to show a note. */
export const onRevealNote = (
	handler: (note: Pick<Note, 'id' | 'date' | 'kind'>) => void
): Promise<UnlistenFn> =>
	listen<{ id: string; date: string; kind: Note['kind'] | null }>('reveal-note', (event) =>
		handler({ ...event.payload, kind: event.payload.kind ?? undefined })
	);

/**
 * Every note and page whose body holds any of `needles` as typed, newest
 * first, such as `[ ]` for the tasks view. An empty needle matches all.
 */
export const notesContaining = (needles: string[]) =>
	invoke<Note[]>('notes_containing', { needles });

/**
 * The spec's signature is `delete_note(id)`; the date comes along until the
 * milestone 2 index can resolve an id to a file on its own.
 */
export const deleteNote = (date: string, id: string) => invoke<void>('delete_note', { date, id });

/**
 * Moves a note of the open space to another, onto the same day, with the
 * files it links.
 */
export const moveNote = (date: string, id: string, space: string) =>
	invoke<void>('move_note', { date, id, space });

/** Today in the local timezone, matching how the backend picks a daily file. */
export const today = () => invoke<string>('today');

export const hideCapture = () => invoke<void>('hide_capture');

export interface TrayLabels {
	newNote: string;
	open: string;
	settings: string;
	quit: string;
}

/** The tray menu is built in Rust, so its wording is sent over in the open language. */
export const setTrayLabels = (labels: TrayLabels) => invoke<void>('set_tray_labels', { labels });

/** Reparses every markdown file and replaces the cache. Returns the note count. */
export const rebuildIndex = () => invoke<number>('rebuild_index');

export const onNoteUpdated = (handler: (id: string) => void): Promise<UnlistenFn> =>
	listen<{ id: string }>('note-updated', (event) => handler(event.payload.id));

/** Fired after a rebuild, and after the watcher picks up an external edit. */
export const onIndexRebuilt = (handler: () => void): Promise<UnlistenFn> =>
	listen('index-rebuilt', () => handler());

/** Fired by the backend each time the capture window is brought up. */
export const onCaptureShown = (handler: () => void): Promise<UnlistenFn> =>
	listen('capture-shown', () => handler());

/** EmbeddingGemma-300M at Q8_0, which finds notes by meaning. */
export const EMBEDDING_SIZE = '~330 MB';

export interface EmbeddingModelInfo {
	installed: boolean;
	/** Percent done while a download runs, null otherwise. */
	downloading: number | null;
}

export const embeddingModelInfo = () => invoke<EmbeddingModelInfo>('embedding_model_info');

export const downloadEmbeddingModel = () => invoke<void>('download_embedding_model');

export type EmbeddingStatus =
	{ state: 'downloading'; percent: number } | { state: 'installed' } | { state: 'absent' };

export const onEmbeddingStatus = (
	handler: (status: EmbeddingStatus) => void
): Promise<UnlistenFn> =>
	listen<EmbeddingStatus>('embedding-status', (event) => handler(event.payload));

/** Replace a note's body. */
export const updateNote = (date: string, id: string, body: string) =>
	invoke<Note>('update_note', { date, id, body });

/** Start a page on a day, today's unless another is given. The title is required. */
export const createPage = (title: string, body: string, date?: string) =>
	invoke<Note>('create_page', { title, body, date });

/** Every page of the open space, newest first. */
export const listPages = () => invoke<Note[]>('list_pages');

/** A page read afresh from its file. */
export const getPage = (id: string) => invoke<Note>('get_page', { id });

/** Replace a page's text, which may be empty. */
export const updatePage = (id: string, body: string) => invoke<Note>('update_page', { id, body });

/**
 * The page view closed. Saves while it was open leave the embedding model
 * alone; this sends the page to it when its text changed.
 */
export const finishPage = (id: string) => invoke<void>('finish_page', { id });

/** Retitle a page, which renames its file and rewrites its stub. */
export const renamePage = (id: string, title: string) => invoke<Note>('rename_page', { id, title });

/** Remove a page's stub from `date`, the day showing it, then its file. */
export const deletePage = (date: string, id: string) => invoke<void>('delete_page', { date, id });

/**
 * Moves a page of the open space to another, onto the same day, with its
 * file, its stub and the files it links. `date` is the day showing it.
 */
export const movePage = (date: string, id: string, space: string) =>
	invoke<void>('move_page', { date, id, space });

/** Turn a note into a page with that title. The page has a new id. */
export const noteToPage = (date: string, id: string, title: string) =>
	invoke<Note>('note_to_page', { date, id, title });

/**
 * From the capture window: hide it and open a new page with this text in the
 * main window, which opens `space` first when it is another.
 */
export const captureToPage = (body: string, space?: string) =>
	invoke<void>('capture_to_page', { body, space });

/** Fired in the main window when the capture window hands it a draft. */
export const onNewPage = (handler: (body: string) => void): Promise<UnlistenFn> =>
	listen<{ body: string }>('new-page', (event) => handler(event.payload.body));

export interface Settings {
	root: string;
	captureHotkey: string;
	/** Hide the capture window on save rather than showing "Saved" first. */
	hideImmediately: boolean;
	/** Accent preset name, see ACCENTS in appearance.ts. */
	accentColor: string;
	/** Font preset name, see FONTS in appearance.ts. */
	fontFamily: string;
	/** Base text size in pixels. */
	fontSize: number;
	/** Corner radius in rem. */
	radius: number;
	/** 'system' follows the OS setting. */
	theme: 'dark' | 'light' | 'system';
	/** A locale such as 'fr', or 'system' to follow the OS language. */
	language: Language;
	/** The first-run walkthrough has been finished or skipped. */
	onboarded: boolean;
	/** The app version whose release notes were last shown; null from 0.1.0. */
	lastSeenVersion: string | null;
}

export interface SettingsView extends Settings {
	/** The root this run is using; differs from `root` until the next launch. */
	activeRoot: string;
}

export const getSettings = () => invoke<SettingsView>('get_settings');

export const setSettings = (settings: Settings) =>
	invoke<SettingsView>('set_settings', { settings });

/** Relaunch the app, which is how a new notes root takes effect. */
export const restartApp = () => invoke<void>('restart_app');

export const onSettingsChanged = (handler: (settings: Settings) => void): Promise<UnlistenFn> =>
	listen<Settings>('settings-changed', (event) => handler(event.payload));

/** Fired by the tray's Settings entry. */
export const onOpenSettings = (handler: () => void): Promise<UnlistenFn> =>
	listen('open-settings', () => handler());

export interface SpaceSummary {
	/** Also the space's folder name under `spaces/`. */
	name: string;
	/**
	 * How many notes it holds, counted when it was last open. Null for a space
	 * never open since, such as a folder made by hand.
	 */
	notes: number | null;
}

export interface SpacesView {
	/** The open space, which every note command acts on. */
	active: string;
	/** By name. */
	spaces: SpaceSummary[];
}

export const listSpaces = () => invoke<SpacesView>('list_spaces');

/** Makes the space and opens it. */
export const createSpace = (name: string) => invoke<SpacesView>('create_space', { name });

/** Renames the space's folder too. */
export const renameSpace = (name: string, newName: string) =>
	invoke<SpacesView>('rename_space', { name, newName });

/** Moves the space's folder to `.scratchnote/trash/`, so nothing is lost. */
export const deleteSpace = (name: string) => invoke<SpacesView>('delete_space', { name });

export const setActiveSpace = (name: string) => invoke<SpacesView>('set_active_space', { name });

/** Shows the space's folder in the system file manager. */
export const openSpaceFolder = (name: string) => invoke<void>('open_space_folder', { name });

/** Opens a web or mail link from a note in the system browser or mail client. */
export const openLink = (url: string) => invoke<void>('open_link', { url });

/** A file copied into the open space's `attachments/` folder (SPEC 3.7). */
export interface Attachment {
	/** The name it came with, for the link's text. */
	name: string;
	/** From the space's folder: `attachments/2026/2026-09-28 shot.png`. */
	path: string;
}

/** Copies files from disk into the open space, or the one named, in order. */
export const addAttachments = (paths: string[], space?: string) =>
	invoke<Attachment[]>('add_attachments', { paths, space });

/**
 * A pasted file has no path, so its bytes go over as they are, and its name
 * and the space it goes into, when not the open one, in headers.
 */
export const saveAttachment = async (file: File, space?: string) =>
	invoke<Attachment>('save_attachment', new Uint8Array(await file.arrayBuffer()), {
		headers: {
			'x-name': encodeURIComponent(file.name || 'image.png'),
			...(space === undefined ? {} : { 'x-space': encodeURIComponent(space) })
		}
	});

/**
 * Moves attachments from one space into another, for a draft that goes
 * there instead. Resolves to where each one went, in order.
 */
export const moveAttachments = (from: string, to: string, paths: string[]) =>
	invoke<string[]>('move_attachments', { from, to, paths });

/** Opens an attachment of the open space in its own app. */
export const openAttachment = (path: string) => invoke<void>('open_attachment', { path });

/** Fired to every window when a space is opened, made, renamed or deleted. */
export const onSpacesChanged = (handler: (view: SpacesView) => void): Promise<UnlistenFn> =>
	listen<SpacesView>('spaces-changed', (event) => handler(event.payload));

/** A plugin's `manifest.json`, as Obsidian's (SPEC 4.9). */
export interface PluginManifest {
	id: string;
	name: string;
	version: string;
	/** The oldest app version it runs on. */
	minAppVersion?: string | null;
	description: string;
	author: string;
	authorUrl?: string | null;
}

/** What `plugins.json` holds: community plugins start off (SPEC 3.9). */
export interface PluginState {
	community: boolean;
	/** Community plugins switched on, by id. */
	enabled: string[];
	/** Core plugins switched off, by id; they are on unless listed. */
	coreDisabled: string[];
	/** Core plugins that start off, switched on, by id. */
	coreEnabled: string[];
}

export interface PluginsView extends PluginState {
	/** Every installed community plugin, by name. */
	installed: PluginManifest[];
}

/** One plugin the registry lists (SPEC 4.10). */
export interface RegistryEntry {
	id: string;
	name: string;
	author: string;
	description: string;
	/** `owner/name` on GitHub. */
	repo: string;
}

export interface PluginDetails {
	/** At the head of its repository: the latest version. */
	manifest: PluginManifest;
	readme: string | null;
}

export const pluginsView = () => invoke<PluginsView>('plugins_view');

/** Every window hears of it and loads or unloads plugins to match. */
export const setPlugins = (plugins: PluginState) => invoke<PluginsView>('set_plugins', { plugins });

/** Refused while community plugins are off or this one is not enabled. */
export const pluginCode = (id: string) =>
	invoke<{ main: string; styles: string | null }>('plugin_code', { id });

/** What the plugin saved last, null before it ever did. */
export const pluginData = (id: string, core: boolean) =>
	invoke<unknown>('plugin_data', { id, core });

export const savePluginData = (id: string, core: boolean, data: unknown) =>
	invoke<void>('save_plugin_data', { id, core, data });

export const browsePlugins = () => invoke<RegistryEntry[]>('browse_plugins');

export const pluginDetails = (repo: string) => invoke<PluginDetails>('plugin_details', { repo });

/** The latest release, over an older one if installed; its data is kept. */
export const installPlugin = (repo: string, id: string) =>
	invoke<PluginsView>('install_plugin', { repo, id });

/** Its files and data go, and it is switched off. */
export const uninstallPlugin = (id: string) => invoke<PluginsView>('uninstall_plugin', { id });

/** Fired to every window when plugins are installed, removed, or switched. */
export const onPluginsChanged = (handler: (view: PluginsView) => void): Promise<UnlistenFn> =>
	listen<PluginsView>('plugins-changed', (event) => handler(event.payload));

/** Fired when a plugin saved its data, with the window that saved it. */
export const onPluginDataChanged = (
	handler: (change: { id: string; window: string }) => void
): Promise<UnlistenFn> =>
	listen<{ id: string; window: string }>('plugin-data-changed', (event) => handler(event.payload));
