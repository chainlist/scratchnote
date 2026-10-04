import { listThreads, noteMap } from '#lib/api.js';
import type { PageLoad } from './$types';

/** Every note's place on the map and the threads that colour them, read again
 *  as notes, threads or places change. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	depends('app:threads');
	depends('app:map');
	const [notes, { threads }] = await Promise.all([noteMap(), listThreads()]);
	return { notes, threads };
};
