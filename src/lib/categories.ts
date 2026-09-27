import names from '$lib/category-names.json';
import { getLocale } from '$lib/paraglide/runtime';

/** Every known category, by language, row for row the same in each. */
const table: Record<string, string[]> = names;
const rows = new Map(table.en.map((name, row) => [name, row]));

/**
 * A category's name in the interface language. Categories are stored in
 * English; one the table does not know, such as one typed by hand, reads as
 * written.
 */
export function categoryLabel(name: string): string {
	const row = rows.get(name);
	return (row !== undefined && table[getLocale()]?.[row]) || name;
}

/**
 * A query as the backend reads it: `#` tokens typed in the interface
 * language become the English names categories are stored under. Anything
 * else, English names included, is left as it is.
 */
export function storedQuery(query: string): string {
	const labels = table[getLocale()] ?? table.en;
	return query
		.split(' ')
		.map((token) => {
			if (!token.startsWith('#')) return token;
			const row = labels.indexOf(token.slice(1).toLowerCase());
			return row === -1 ? token : `#${table.en[row]}`;
		})
		.join(' ');
}
