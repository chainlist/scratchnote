import { event, invoke } from './invoke.js';
import type { Note } from './notes.js';

/** The notes about one thing, gathered across days (SPEC 6.4). */
export interface Thread {
	/** The id of the note it started with, which it keeps, after `@key:`
	 *  for a thread found among the notes of a name mentioned. */
	id: string;
	/** The key of that name, or null for a thread of the general scope,
	 *  among the notes that mention no name. */
	scope: string | null;
	/** That name as typed. */
	mention: string | null;
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
	/** The days of its notes, each once, oldest first. */
	days: string[];
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
 * Put notes in a thread, or a new one when `into` is null, made among the
 * notes of the name `scope` keys, or the general scope's without one. They
 * stay there whatever they score. Resolves to the thread's id.
 */
export const putInThread = (notes: string[], into: string | null, scope?: string | null) =>
	invoke<string>('put_in_thread', { notes, into, scope });

/** Put every note of thread `from` in thread `into`. */
export const mergeThreads = (from: string, into: string) =>
	invoke<void>('merge_threads', { from, into });

/** Take back the last merge, dismissal or note taken out, while nothing has
 *  changed the threads since. Resolves to whether it did. */
export const undoThreadChange = () => invoke<boolean>('undo_thread_change');

/** Fired when the notes were placed in threads anew, or a thread renamed. */
export const onThreadsChanged = event('threads-changed');
