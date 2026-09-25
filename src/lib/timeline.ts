import type { Note } from '$lib/api';

/** What the main page's timeline lists: a day, or something in its place. */
export type Timeline =
	| { kind: 'day' }
	/** Every tag of the space. */
	| { kind: 'tags' }
	/** The notes a query matches, from the command center's "See all". */
	| { kind: 'search'; query: string }
	/** The notes closest in meaning to this one. */
	| { kind: 'similar'; note: Note };
