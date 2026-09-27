import type { Note } from '$lib/api';

/** What the main page's timeline lists: a day, or something in its place. */
export type Timeline =
	| { kind: 'day' }
	/** A month of days, to pick one from. */
	| { kind: 'calendar' }
	/** The notes a query matches, from the command center's "See all". */
	| { kind: 'search'; query: string }
	/** The notes closest in meaning to this one. */
	| { kind: 'similar'; note: Note }
	/** One page to read and write; a null id is a new one, saved once it has a title. */
	| { kind: 'page'; id: string | null };
