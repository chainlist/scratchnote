import { resolve } from '$app/paths';
import type { Note, Thread } from '#lib/api.js';
import { hueOf, mentionHue } from '#lib/mentions.js';
import { noteTitle } from '#lib/markdown.js';
import { m } from '#lib/paraglide/messages.js';

/**
 * A thread's name, the same wherever it shows: the user's title, else the
 * first line of its first note as words, else "Thread" while that note is
 * not read yet.
 */
export function threadName(thread: Pick<Thread, 'title'>, first?: Pick<Note, 'subject' | 'body'>) {
	return thread.title ?? ((first && noteTitle(first)) || m.thread_untitled());
}

/**
 * The key of the name a thread was found among the notes of, from its id,
 * `@key:note`, or null for the general scope (SPEC 6.4). As the backend's
 * `scope_of`.
 */
export function threadScope(id: string): string | null {
	if (!id.startsWith('@')) return null;
	const end = id.indexOf(':');
	return end < 0 ? null : id.slice(1, end);
}

/** A thread's own hue (oklch), the same in every view, its pin's and its
 *  dots on the map too: its name's, for a thread found among a name's
 *  notes, else one of its own from its id. */
export function threadHue(id: string) {
	const scope = threadScope(id);
	return scope === null ? hueOf(id) : mentionHue(scope);
}

/** How many suggested threads the Threads page draws at a time. */
export const SUGGESTED_PAGE = 30;

/** The notes a thread is told apart by: its first two. */
export const leadNotes = (threads: Thread[]) =>
	threads.flatMap((thread) => thread.notes.slice(0, 2));

/** The view of a thread, in the timeline's place. */
export const threadHref = (id: string) => resolve(`thread/${encodeURIComponent(id)}/`);

/** The tasks still open in the notes, as they come, each with its note. */
export const openTasks = (notes: Note[]) =>
	notes.flatMap((note) =>
		note.body.split('\n').flatMap((line) => {
			const task = line.match(/^\s*[-*+]\s+\[ \]\s+(.+)$/)?.[1];
			return task ? [{ note, task: noteTitle({ subject: null, body: task }) }] : [];
		})
	);
