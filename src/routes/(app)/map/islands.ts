import type { MapCategory, MapNote } from '#lib/api.js';

/** A category as the map shows it: its notes, the box they span and its name. */
export interface Island {
	notes: MapNote[];
	minX: number;
	maxX: number;
	minY: number;
	maxY: number;
	name: string;
}

/** The categories shown at `level`, each with its notes: a note is in the
 *  smallest category it is in that shows at that level, if any. */
export function islands(
	notes: MapNote[],
	categories: Map<number, MapCategory>,
	level: number
): Island[] {
	const shown = new Map<MapCategory, MapNote[]>();
	for (const note of notes) {
		let category = note.category === null ? undefined : categories.get(note.category);
		while (category && category.high < level) {
			category = category.parent === null ? undefined : categories.get(category.parent);
		}
		if (!category || category.low > level) continue;
		const group = shown.get(category);
		if (group) group.push(note);
		else shown.set(category, [note]);
	}
	return [...shown].map(([category, notes]) => {
		const xs = notes.map((note) => note.x);
		const ys = notes.map((note) => note.y);
		return {
			notes,
			minX: Math.min(...xs),
			maxX: Math.max(...xs),
			minY: Math.min(...ys),
			maxY: Math.max(...ys),
			name: category.name
		};
	});
}
