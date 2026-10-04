import { listThreads, mapCategories, noteMap } from '#lib/api.js';
import type { PageLoad } from './$types';

/** Every note's place on the map, its categories and the threads that colour
 *  the notes, read again as notes, threads or places change. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	depends('app:threads');
	depends('app:map');
	const [notes, categories, { threads }] = await Promise.all([
		noteMap(),
		mapCategories(),
		listThreads()
	]);
	return { notes, categories, threads };
};
