import { invoke } from './invoke.js';
import type { Note } from './notes.js';

/** A name the notes mention with `@name` (SPEC 3.10). */
export interface MentionSummary {
	/** As the newest note mentioning it types it. */
	name: string;
	/** Lowercase: case does not count. */
	key: string;
	/** How many notes and pages mention it. */
	notes: number;
	/** The day of the last. */
	last: string;
	/** The days of those notes, each once, oldest first. */
	days: string[];
	/** Pinned to the left edge, as a project in hand is (SPEC 3.13). */
	pinned: boolean;
}

/** Every name the open space mentions, most mentioned first. None for
 *  another space, which is not read. */
export const listMentions = (space?: string) =>
	invoke<MentionSummary[]>('list_mentions', { space });

/** The notes and pages that mention a name, newest first, and the name as
 *  the newest of them types it: null when none does. */
export const notesMentioning = (name: string) =>
	invoke<{ name: string | null; notes: Note[] }>('notes_mentioning', { name });
