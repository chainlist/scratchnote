import { search } from '$lib/api';
import type { PageLoad } from './$types';

// The query is in the URL's search, which prerendering cannot know.
export const prerender = false;

/** The notes a query matches, from the command center's "See all". */
export const load: PageLoad = async ({ url, depends }) => {
	depends('app:notes');
	const query = url.searchParams.get('q') ?? '';
	return { query, results: await search(query) };
};
