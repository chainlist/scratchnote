import { threadCards } from '#lib/api.js';
import type { PageLoad } from './$types';

/** Every thread of the space, the one written in last first, read again as notes or threads change. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	depends('app:threads');
	return { threads: await threadCards() };
};
