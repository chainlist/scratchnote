import { listMentions } from '#lib/api.js';
import type { PageLoad } from './$types';

/** Every name the space mentions, read again as the notes change. */
export const load: PageLoad = async ({ depends }) => {
	depends('app:notes');
	return { mentions: await listMentions() };
};
