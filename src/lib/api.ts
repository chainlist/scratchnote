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

/** Today in the local timezone, matching how the backend picks a daily file. */
export const today = () => invoke<string>('today');

export const hideCapture = () => invoke<void>('hide_capture');

export const onNoteUpdated = (handler: (id: string) => void): Promise<UnlistenFn> =>
	listen<{ id: string }>('note-updated', (event) => handler(event.payload.id));

/** Fired by the backend each time the capture window is brought up. */
export const onCaptureShown = (handler: () => void): Promise<UnlistenFn> =>
	listen('capture-shown', () => handler());
