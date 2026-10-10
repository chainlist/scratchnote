/** Folded for matching: `é` finds `e`, and case does not count. */
const fold = (text: string) =>
	text
		.normalize('NFD')
		.replace(/\p{Diacritic}/gu, '')
		.toLowerCase();

/** The words of what was typed, folded. */
export const queryWords = (query: string) => fold(query).split(/\s+/).filter(Boolean);

/** `text` holds every one of `words`, as `queryWords` gives them. */
export function hasEvery(text: string, words: string[]) {
	const folded = fold(text);
	return words.every((word) => folded.includes(word));
}
