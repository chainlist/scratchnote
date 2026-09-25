/** The `#tag` tokens of a search query, without the `#`, lowercased. */
export function queryTags(query: string): string[] {
	return query
		.split(/\s+/)
		.filter((token) => token.startsWith('#'))
		.map((token) => token.slice(1).toLowerCase());
}

/** The words of a search query, its tags left out. */
export function queryWords(query: string): string {
	return query
		.split(/\s+/)
		.filter((token) => token && !token.startsWith('#'))
		.join(' ');
}

/**
 * The query once a tag clicked in the results is applied. A plain click
 * filters on this tag alone, or clears it if it already was the only one;
 * `additive` adds the tag or takes it out again. Words are kept.
 */
export function toggleTag(query: string, tag: string, additive: boolean): string {
	const tokens = query.split(/\s+/).filter(Boolean);
	const kept = tokens.filter((token) => token.toLowerCase() !== `#${tag}`);
	const on = kept.length !== tokens.length;
	if (additive) return (on ? kept : [...tokens, `#${tag}`]).join(' ');
	const plain = tokens.filter((token) => !token.startsWith('#'));
	const alone = on && queryTags(query).length === 1;
	return (alone ? plain : [...plain, `#${tag}`]).join(' ');
}
