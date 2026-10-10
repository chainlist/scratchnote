import { invoke } from './invoke.js';
import type { Note } from './notes.js';

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
