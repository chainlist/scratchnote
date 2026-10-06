import { resolve } from '$app/paths';
import type { Thread } from '#lib/api.js';

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

/** How many suggested threads the Threads page draws at a time. */
export const SUGGESTED_PAGE = 30;

/** The notes a thread is told apart by: its first two. */
export const leadNotes = (threads: Thread[]) =>
	threads.flatMap((thread) => thread.notes.slice(0, 2));

/** The view of a thread, in the timeline's place. */
export const threadHref = (id: string) => resolve(`thread/${encodeURIComponent(id)}/`);
