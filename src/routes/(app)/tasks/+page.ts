import { taskNotes } from '$lib/api';
import type { PageLoad } from './$types';

/** The notes and pages with tasks, newest first; the view keeps the open ones. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	return { notes: await taskNotes() };
};
