import { listPages } from '$lib/api';
import type { PageLoad } from './$types';

/** Every page of the space, newest first. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	return { pages: await listPages() };
};
