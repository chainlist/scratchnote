import { listen, type UnlistenFn } from '@tauri-apps/api/event';
import { event, invoke } from './invoke.js';

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
	/** The user cleared its day ahead, so none is read from its text. */
	ahead_off?: boolean;
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

/** One note or page with its text, or null once it is gone. */
export const getNote = (id: string) => invoke<Note | null>('get_note', { id });

/** Some notes or pages with their text, in the order asked, those gone left out. */
export const getNotes = (ids: string[]) => invoke<Note[]>('get_notes', { ids });

/** Replace a note's body. */
export const updateNote = (date: string, id: string, body: string) =>
	invoke<Note>('update_note', { date, id, body });

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

/** Reparses every markdown file and replaces the cache. Returns the note count. */
export const rebuildIndex = () => invoke<number>('rebuild_index');

/** Opens a web or mail link from a note in the system browser or mail client. */
export const openLink = (url: string) => invoke<void>('open_link', { url });

export const onNoteUpdated = (handler: (id: string) => void): Promise<UnlistenFn> =>
	listen<{ id: string }>('note-updated', (e) => handler(e.payload.id));

/** Fired after a rebuild, and after the watcher picks up an external edit. */
export const onIndexRebuilt = event('index-rebuilt');
