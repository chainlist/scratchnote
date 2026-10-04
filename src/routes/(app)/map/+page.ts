import { listThreads, mapCategories, noteMap } from '#lib/api.js';
import type { PageLoad } from './$types';

/** Every note's place on the map, its categories and the threads that colour
 *  the notes, read again as threads or places change. Not as notes are
 *  saved: a note joins the map, moves on it or leaves it only once the embed
 *  task has placed it, which `app:map` follows. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:threads');
	depends('app:map');
	const [notes, categories, { threads }] = await Promise.all([
		noteMap(),
		mapCategories(),
		listThreads()
	]);
	return { notes, categories, threads };
};
