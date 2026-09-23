import { invoke } from '@tauri-apps/api/core';
import { listen, type UnlistenFn } from '@tauri-apps/api/event';

export type NoteStatus = 'pending' | 'done' | 'failed' | 'manual';

export interface Note {
	id: string;
	date: string;
	time: string;
	file: string;
	subject: string | null;
	summary: string | null;
	tags: string[];
	status: NoteStatus;
	hash: string;
	body: string;
}

export interface DaySummary {
	date: string;
	count: number;
}

/** Returns null when the body was empty, which is a no-op rather than an error. */
export const saveNote = (body: string) => invoke<Note | null>('save_note', { body });

export const getDay = (date: string) => invoke<Note[]>('get_day', { date });

export const listDays = () => invoke<DaySummary[]>('list_days');

/** Every tag in use and how many notes carry it, most used first. */
export const listTags = () => invoke<[string, number][]>('list_tags');

/** Words must all appear; `#tag` tokens are AND filters. Newest first. */
export const search = (query: string) => invoke<Note[]>('search', { query });

/**
 * The spec's signature is `delete_note(id)`; the date comes along until the
 * milestone 2 index can resolve an id to a file on its own.
 */
export const deleteNote = (date: string, id: string) => invoke<void>('delete_note', { date, id });

/** Today in the local timezone, matching how the backend picks a daily file. */
export const today = () => invoke<string>('today');

export const hideCapture = () => invoke<void>('hide_capture');

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

export type ModelState = 'absent' | 'downloading' | 'loaded' | 'idle';

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

export const MODEL_CHOICES: { variant: ModelVariant; name: string; size: string; note: string }[] =
	[
		{
			variant: 'default',
			name: 'Default',
			size: '~2.5 GB',
			note: 'Qwen3 4B. Better labels, wants more RAM.'
		},
		{
			variant: 'light',
			name: 'Light',
			size: '~1.1 GB',
			note: 'Qwen3 1.7B. Quicker, kinder to a small machine.'
		}
	];

export const modelStatus = () => invoke<ModelStatus>('model_status');

export interface ModelInfo {
	/** The file enrichment loads, or null when there is nothing usable. */
	activePath: string | null;
	/** Null for a custom GGUF file. */
	activeVariant: ModelVariant | null;
	default: InstalledModel | null;
	light: InstalledModel | null;
}

export const modelInfo = () => invoke<ModelInfo>('model_info');

/** GPUs the model can run on. Empty when the machine has no usable device. */
export const gpuDevices = () => invoke<string[]>('gpu_devices');

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

export const downloadModel = (variant: ModelVariant) => invoke<void>('download_model', { variant });

export const retryEnrichment = (date: string, id: string) =>
	invoke<void>('retry_enrichment', { date, id });

/** What the note card's editor hands back. */
export interface NoteEdit {
	body: string;
	subject: string;
	tags: string[];
}

/** Replace a note's body. A changed body is re-enriched unless the note is manual. */
export const updateNote = (date: string, id: string, body: string) =>
	invoke<Note>('update_note', { date, id, body });

/** Set subject and tags by hand, which makes the note manual. Omitted fields are kept. */
export const updateNoteMeta = (
	date: string,
	id: string,
	meta: { subject?: string; tags?: string[] }
) => invoke<Note>('update_note_meta', { date, id, ...meta });

/** Fired when a note has been labelled and written back. */
export const onNoteEnriched = (handler: (id: string) => void): Promise<UnlistenFn> =>
	listen<{ id: string }>('note-enriched', (event) => handler(event.payload.id));

export const onModelStatus = (handler: (status: ModelStatus) => void): Promise<UnlistenFn> =>
	listen<ModelStatus>('model-status', (event) => handler(event.payload));

export interface Settings {
	root: string;
	captureHotkey: string;
	/** Hide the capture window on save rather than showing "Saved" first. */
	hideImmediately: boolean;
	modelVariant: ModelVariant;
	/** A GGUF file of the user's own, used instead of modelVariant. */
	modelPath: string | null;
	/** Unload the model after this many idle minutes; 0 keeps it loaded. */
	idleUnloadMinutes: number;
	/** Run the model on the GPU when one is available. */
	useGpu: boolean;
}

export interface SettingsView extends Settings {
	/** The root this run is using; differs from `root` until the next launch. */
	activeRoot: string;
}

export const getSettings = () => invoke<SettingsView>('get_settings');

export const setSettings = (settings: Settings) =>
	invoke<SettingsView>('set_settings', { settings });

export const getAliases = () => invoke<Record<string, string>>('get_aliases');

/** Returns the aliases as cleaned and saved. */
export const setAliases = (aliases: Record<string, string>) =>
	invoke<Record<string, string>>('set_aliases', { aliases });

export const onSettingsChanged = (handler: (settings: Settings) => void): Promise<UnlistenFn> =>
	listen<Settings>('settings-changed', (event) => handler(event.payload));

/** Fired by the tray's Settings entry. */
export const onOpenSettings = (handler: () => void): Promise<UnlistenFn> =>
	listen('open-settings', () => handler());
