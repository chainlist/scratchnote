import { getDay, similarNotes } from '#lib/api.js';
import type { PageLoad } from './$types';

// Any note, so none is known ahead to prerender; the fallback page serves it.
export const prerender = false;

/** The notes closest in meaning to a note, which is read from its day. */
export const load: PageLoad = async ({ params, depends }) => {
	depends('app:notes');
	const [day, similar] = await Promise.all([getDay(params.date), similarNotes(params.id)]);
	return { note: day.find((note) => note.id === params.id), similar };
};
