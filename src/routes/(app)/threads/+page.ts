import { getNotes, listThreads } from '#lib/api.js';
import { leadNotes, SUGGESTED_PAGE } from '#lib/notes/threads.js';
import type { PageLoad } from './$types';

/** Every thread of the space, the one written in last first, with the first
 *  notes of those drawn at once: the user's and the first suggestions. Read
 *  again as notes or threads change. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	depends('app:threads');
	const { threads } = await listThreads();
	// As the backend orders strings, by code unit.
	const after = (a: string, b: string) => (a < b ? 1 : a > b ? -1 : 0);
	threads.sort((a, b) => after(a.until, b.until) || after(a.id, b.id));
	const drawn = [
		...threads.filter((thread) => thread.kept),
		...threads.filter((thread) => !thread.kept).slice(0, SUGGESTED_PAGE)
	];
	return { threads, notes: await getNotes(leadNotes(drawn)) };
};
