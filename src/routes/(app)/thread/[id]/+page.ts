import { getThread } from '#lib/api.js';
import type { PageLoad } from './$types';

// Any thread, so none is known ahead to prerender; the fallback page serves it.
export const prerender = false;

/** A thread and its notes, oldest first, read again as its notes or threads change. */
export const load: PageLoad = async ({ params, depends }) => {
	depends('app:notes');
	depends('app:threads');
	return { found: await getThread(params.id) };
};
