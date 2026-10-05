import { resolve } from '$app/paths';

/**
 * The key of the name a thread was found among the notes of, from its id,
 * `@key:note`, or null for the general scope (SPEC 6.4). As the backend's
 * `scope_of`.
 */
export function threadScope(id: string): string | null {
	if (!id.startsWith('@')) return null;
	const end = id.indexOf(':');
	return end < 0 ? null : id.slice(1, end);
}

/** The view of a thread, in the timeline's place. */
export const threadHref = (id: string) => resolve(`thread/${encodeURIComponent(id)}/`);
