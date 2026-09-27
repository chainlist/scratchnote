/** The `#category` tokens of a search query, without the `#`, lowercased. */
export function queryCategories(query: string): string[] {
	return query
		.split(/\s+/)
		.filter((token) => token.startsWith('#'))
		.map((token) => token.slice(1).toLowerCase());
}

/**
 * The query once a category clicked in the results is applied: it filters on
 * this category alone, or clears it if it already was the only one. Words are
 * kept.
 */
export function toggleCategory(query: string, category: string): string {
	const tokens = query.split(/\s+/).filter(Boolean);
	const on = tokens.some((token) => token.toLowerCase() === `#${category}`);
	const plain = tokens.filter((token) => !token.startsWith('#'));
	const alone = on && queryCategories(query).length === 1;
	return (alone ? plain : [...plain, `#${category}`]).join(' ');
}
