import { Channel, invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { storedQuery } from '$lib/categories';
import type { Language } from '$lib/i18n.svelte';
import { m } from '$lib/paraglide/messages';

export type NoteStatus = 'pending' | 'done' | 'failed' | 'manual';

export interface Note {
	id: string;
	date: string;
	time: string;
	file: string;
	subject: string | null;
	category: string | null;
	status: NoteStatus;
	hash: string;
	body: string;
}

export interface DaySummary {
	date: string;
	count: number;
}

/**
 * Saves to today unless a date is given. Returns null when the body was
 * empty, which is a no-op rather than an error.
 */
export const saveNote = (body: string, date?: string) =>
	invoke<Note | null>('save_note', { body, date });

export const getDay = (date: string) => invoke<Note[]>('get_day', { date });

export const listDays = () => invoke<DaySummary[]>('list_days');

/**
 * The categories notes are filed under, with how many carry each, most used
 * first. `#category` in a search filters on one.
 */
export const listCategories = () => invoke<[string, number][]>('list_categories');

/** Every category on the list, used or not, in the list's order. */
export const categoryNames = () => invoke<string[]>('category_names');

/** Words must all appear; a `#category` token filters on the category. Newest first. */
export const search = (query: string) => invoke<Note[]>('search', { query: storedQuery(query) });

/** Notes close in meaning that the words miss. Empty without the embedding model. */
export const searchMeaning = (query: string) =>
	invoke<Note[]>('search_meaning', { query: storedQuery(query) });

/**
 * The notes closest in meaning to this one, best first. Empty while the note
 * has no vector yet, or without the embedding model.
 */
export const similarNotes = (id: string) => invoke<Note[]>('similar_notes', { id });

/** One line of `index.jsonl`, parsed. The index holds no bodies. */
export type IndexEntry = Omit<Note, 'body'>;

/** One turn of a chat. */
export interface ChatMessage {
	role: 'user' | 'assistant';
	content: string;
}

/** What a reply sends, in order: the notes, pieces of text, then done. */
export type ChatEvent =
	/** The index the model reads, numbered as it sees it: note n is notes[n - 1]. */
	| { kind: 'notes'; notes: IndexEntry[] }
	| { kind: 'token'; text: string }
	/** The reply is complete or was stopped; the note numbers it cites. */
	| { kind: 'done'; cited: number[] };

/**
 * Reply to the last message, streamed to `onEvent`. The model reads the open
 * space's `index.jsonl` and nothing else. Resolves once the reply is done.
 */
export function chat(messages: ChatMessage[], onEvent: (event: ChatEvent) => void) {
	const channel = new Channel<ChatEvent>();
	channel.onmessage = onEvent;
	return invoke<void>('chat', { messages, onEvent: channel });
}

/** End the reply being written; it finishes with what it has so far. */
export const stopChat = () => invoke<void>('stop_chat');

/** Read the open space's index into the model ahead of the first message. */
export const warmChat = () => invoke<void>('warm_chat');

/**
 * A reply split into text and citations: `[12]` and `[3, 7]` become the note
 * numbers they name. Anything else in brackets stays text.
 */
export function splitCitations(text: string): (string | number[])[] {
	const parts: (string | number[])[] = [];
	let last = 0;
	for (const match of text.matchAll(/\[(\d+(?:\s*,\s*\d+)*)\]/g)) {
		if (match.index > last) parts.push(text.slice(last, match.index));
		parts.push(match[1].split(',').map((n) => Number(n.trim())));
		last = match.index + match[0].length;
	}
	if (last < text.length) parts.push(text.slice(last));
	return parts;
}

/**
 * The spec's signature is `delete_note(id)`; the date comes along until the
 * milestone 2 index can resolve an id to a file on its own.
 */
export const deleteNote = (date: string, id: string) => invoke<void>('delete_note', { date, id });

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

/**
 * Rebuild the index and queue every note for a fresh subject and category,
 * hand-edited ones included. Returns how many notes were queued.
 */
export const regenerateAll = () => invoke<number>('regenerate_all');

export const onNoteUpdated = (handler: (id: string) => void): Promise<UnlistenFn> =>
	listen<{ id: string }>('note-updated', (event) => handler(event.payload.id));

/** Fired after a rebuild, and after the watcher picks up an external edit. */
export const onIndexRebuilt = (handler: () => void): Promise<UnlistenFn> =>
	listen('index-rebuilt', () => handler());

/** Fired by the backend each time the capture window is brought up. */
export const onCaptureShown = (handler: () => void): Promise<UnlistenFn> =>
	listen('capture-shown', () => handler());

/** 'disabled' is the settings switch, whether or not a model is on disk. */
export type ModelState = 'absent' | 'downloading' | 'loaded' | 'idle' | 'disabled';

export interface ModelStatus {
	state: ModelState;
	/** Only present while downloading. */
	percent?: number;
}

export interface InstalledModel {
	repo: string;
	file: string;
	/** Hugging Face commit the weights came from; quant repos are re-uploaded in place. */
	revision: string;
	sha256: string;
}

export type ModelVariant = 'default' | 'light';

/** Name and note are messages, called where they are shown so they follow the language. */
export const MODEL_CHOICES: {
	variant: ModelVariant;
	name: () => string;
	size: string;
	note: () => string;
}[] = [
	{
		variant: 'default',
		name: m.model_default_name,
		size: '~2.5 GB',
		note: m.model_default_note
	},
	{
		variant: 'light',
		name: m.model_light_name,
		size: '~1.1 GB',
		note: m.model_light_note
	}
];

export const modelStatus = () => invoke<ModelStatus>('model_status');

/** True while a note is being enriched, loading the model included. */
export const enrichBusy = () => invoke<boolean>('enrich_busy');

export interface ModelInfo {
	/** The file enrichment loads, or null when there is nothing usable. */
	activePath: string | null;
	/** Null for a custom GGUF file. */
	activeVariant: ModelVariant | null;
	default: InstalledModel | null;
	light: InstalledModel | null;
}

export const modelInfo = () => invoke<ModelInfo>('model_info');

/** What the benchmark sends as it runs. */
export type BenchmarkEvent =
	/** The model was not in memory; this is what loading it took. */
	| { kind: 'load'; ms: number }
	/** Sample `done` of `total` is labelled, or failed to be. */
	| { kind: 'note'; done: number; total: number; ms: number; ok: boolean };

/**
 * Label a few fixed sample notes with the model in use, loading it if need
 * be, timing each to `onEvent`. Nothing is written. Resolves once done.
 */
export function benchmarkModel(onEvent: (event: BenchmarkEvent) => void) {
	const channel = new Channel<BenchmarkEvent>();
	channel.onmessage = onEvent;
	return invoke<void>('benchmark_model', { onEvent: channel });
}

/** GPUs the model can run on. Empty when the machine has no usable device. */
export const gpuDevices = () => invoke<string[]>('gpu_devices');

export interface Gpu {
	name: string;
	/** In bytes; zero when the driver does not say. */
	memory: number;
	/** Shares the system memory rather than having its own. */
	integrated: boolean;
}

/** Why the suggested model was picked. */
export type HardwareReason = 'gpu' | 'cpu' | 'small';

export interface Hardware {
	gpus: Gpu[];
	/** System memory in bytes; zero when unknown. */
	memory: number;
	threads: number;
	recommended: ModelVariant;
	reason: HardwareReason;
}

/**
 * The machine and the model it suits, read before any model is on disk for
 * the benchmark to time. An estimate, for the onboarding.
 */
export const systemProfile = () => invoke<Hardware>('system_profile');

export type UpdateCheck =
	| { state: 'upToDate'; revision: string }
	| { state: 'newer'; installed: string; latest: string }
	| { state: 'failed'; reason: string };

/** Only ever called from the settings button (SPEC 5.2). */
export const checkModelUpdate = () => invoke<UpdateCheck>('check_model_update');

/** The old model stays in use until the new one is downloaded and verified. */
export const updateModel = () => invoke<void>('update_model');

export const onModelUpdateProgress = (handler: (percent: number) => void): Promise<UnlistenFn> =>
	listen<{ percent: number }>('model-update', (event) => handler(event.payload.percent));

/**
 * Also starts the embedding model's download once this one is in, when it is
 * not installed yet. Its progress goes out as `embedding-status`.
 */
export const downloadModel = (variant: ModelVariant) => invoke<void>('download_model', { variant });

/** EmbeddingGemma-300M at Q8_0, which lets the chat find notes by meaning. */
export const EMBEDDING_SIZE = '~330 MB';

export interface EmbeddingModelInfo {
	installed: boolean;
	/** Percent done while a download runs, null otherwise. */
	downloading: number | null;
}

export const embeddingModelInfo = () => invoke<EmbeddingModelInfo>('embedding_model_info');

/** Leaves the chat model and its settings alone. */
export const downloadEmbeddingModel = () => invoke<void>('download_embedding_model');

export type EmbeddingStatus =
	{ state: 'downloading'; percent: number } | { state: 'installed' } | { state: 'absent' };

export const onEmbeddingStatus = (
	handler: (status: EmbeddingStatus) => void
): Promise<UnlistenFn> =>
	listen<EmbeddingStatus>('embedding-status', (event) => handler(event.payload));

export const retryEnrichment = (date: string, id: string) =>
	invoke<void>('retry_enrichment', { date, id });

/** What the note editor hands back. */
export interface NoteEdit {
	body: string;
	subject: string;
	/** Empty for none. */
	category: string;
}

/** Replace a note's body. A changed body is re-enriched unless the note is manual. */
export const updateNote = (date: string, id: string, body: string) =>
	invoke<Note>('update_note', { date, id, body });

/**
 * Set subject and category by hand, which makes the note manual so the model
 * leaves it alone from then on. Omitted fields are kept, and an empty
 * category clears it.
 */
export const updateNoteMeta = (
	date: string,
	id: string,
	meta: { subject?: string; category?: string }
) => invoke<Note>('update_note_meta', { date, id, ...meta });

/** Fired when a note has been labelled and written back. */
export const onNoteEnriched = (handler: (id: string) => void): Promise<UnlistenFn> =>
	listen<{ id: string }>('note-enriched', (event) => handler(event.payload.id));

export const onModelStatus = (handler: (status: ModelStatus) => void): Promise<UnlistenFn> =>
	listen<ModelStatus>('model-status', (event) => handler(event.payload));

export const onEnrichBusy = (handler: (busy: boolean) => void): Promise<UnlistenFn> =>
	listen<boolean>('enrich-busy', (event) => handler(event.payload));

/** Which note of how many the worker is on, counting since it last went idle. */
export interface EnrichProgress {
	current: number;
	total: number;
}

/** Null while the worker is idle. */
export const enrichProgress = () => invoke<EnrichProgress | null>('enrich_progress');

/** Fired as the worker starts each note. */
export const onEnrichProgress = (
	handler: (progress: EnrichProgress | null) => void
): Promise<UnlistenFn> =>
	listen<EnrichProgress | null>('enrich-progress', (event) => handler(event.payload));

export interface Settings {
	root: string;
	captureHotkey: string;
	/** Hide the capture window on save rather than showing "Saved" first. */
	hideImmediately: boolean;
	/** Off: no enrichment and no chat, and the model never loads. */
	modelEnabled: boolean;
	modelVariant: ModelVariant;
	/** A GGUF file of the user's own, used instead of modelVariant. */
	modelPath: string | null;
	/** Unload the model after this many idle minutes; 0 keeps it loaded. */
	idleUnloadMinutes: number;
	/** Run the model on the GPU when one is available. */
	useGpu: boolean;
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
	notes: number;
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

/** Fired to every window when a space is opened, made, renamed or deleted. */
export const onSpacesChanged = (handler: (view: SpacesView) => void): Promise<UnlistenFn> =>
	listen<SpacesView>('spaces-changed', (event) => handler(event.payload));
