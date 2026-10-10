import { invoke } from './invoke.js';
import type { Note } from './notes.js';

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

/**
 * Every note and page whose body holds any of `needles` as typed, newest
 * first, such as `[ ]` for the tasks view. An empty needle matches all.
 */
export const notesContaining = (needles: string[]) =>
	invoke<Note[]>('notes_containing', { needles });

/** Notes close in meaning that the words miss. Empty without the embedding model. */
export const searchMeaning = (query: string) => invoke<Note[]>('search_meaning', { query });

/**
 * The notes closest in meaning to this one, best first. Empty while the note
 * has no vector yet, or without the embedding model.
 */
export const similarNotes = (id: string) => invoke<Note[]>('similar_notes', { id });

/**
 * What a draft calls to mind as it is written: the old note it is about,
 * once one stands out (SPEC 6.3), and for a draft that mentions no name, the
 * name it looks like (SPEC 3.10). `exclude` is the note it is an edit of.
 * Nothing for a draft going into a space that is not open, or without the
 * embedding model.
 */
export const draftHints = (text: string, exclude?: string, space?: string) =>
	invoke<{ note: Note | null; mention: string | null }>('draft_hints', { text, exclude, space });
