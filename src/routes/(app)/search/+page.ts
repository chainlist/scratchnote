import { search } from '$lib/api';
import { RESULTS_STEP } from '$lib/query';
import type { PageLoad } from './$types';

// The query is in the URL's search, which prerendering cannot know.
export const prerender = false;

/**
 * The notes a query matches, from the command center's "See all": the first
 * `n`, which each "Show more" raises. It is kept in the URL, so a note that
 * changes while the list is open lists as many again.
 */
export const load: PageLoad = async ({ url, depends }) => {
	depends('app:notes');
	const query = url.searchParams.get('q') ?? '';
	const shown = Math.max(RESULTS_STEP, Math.floor(Number(url.searchParams.get('n'))) || 0);
	return { query, shown, found: await search(query, shown) };
};
