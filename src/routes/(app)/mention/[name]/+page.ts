import { notesMentioning } from '#lib/api.js';
import type { PageLoad } from './$types';

// Any name, so none is known ahead to prerender; the fallback page serves it.
export const prerender = false;

/** The notes and pages that mention a name, newest first, read again as the notes change. */
export const load: PageLoad = async ({ params, depends }) => {
	depends('app:notes');
	return { found: await notesMentioning(params.name) };
};
