import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { event, invoke } from './invoke.js';
import type { Note } from './notes.js';

/** What the app did at launch before its window loaded (dev builds show it). */
export interface Launch {
	/** When the process began, in ms since the Unix epoch. */
	began: number;
	steps: { step: string; ms: number }[];
}

/** The launch's steps, to the first window that asks; null after. */
export const launchSteps = () => invoke<Launch | null>('launch_steps');

export const hideCapture = () => invoke<void>('hide_capture');

/**
 * From the capture window: hide it and open a new page with this text in the
 * main window, which opens `space` first when it is another.
 */
export const captureToPage = (body: string, space?: string) =>
	invoke<void>('capture_to_page', { body, space });

/**
 * From the capture window: hide it, keeping its draft, and show the note in
 * the main window.
 */
export const revealNote = (note: Pick<Note, 'id' | 'date' | 'kind'>) =>
	invoke<void>('reveal_note', { id: note.id, date: note.date, kind: note.kind });

export interface TrayLabels {
	newNote: string;
	open: string;
	settings: string;
	quit: string;
}

/** The tray menu is built in Rust, so its wording is sent over in the open language. */
export const setTrayLabels = (labels: TrayLabels) => invoke<void>('set_tray_labels', { labels });

/** Fired in the main window when the capture window asks it to show a note. */
export const onRevealNote = (
	handler: (note: Pick<Note, 'id' | 'date' | 'kind'>) => void
): Promise<UnlistenFn> =>
	listen<{ id: string; date: string; kind: Note['kind'] | null }>('reveal-note', (e) =>
		handler({ ...e.payload, kind: e.payload.kind ?? undefined })
	);

/** Fired in the main window when the capture window hands it a draft. */
export const onNewPage = (handler: (body: string) => void): Promise<UnlistenFn> =>
	listen<{ body: string }>('new-page', (e) => handler(e.payload.body));

/** Fired by the backend each time the capture window is brought up. */
export const onCaptureShown = event('capture-shown');
