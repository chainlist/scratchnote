import type { MapNote, MentionSummary, Thread } from '#lib/api.js';

/** A point on the map or on the canvas. */
export interface Point {
	x: number;
	y: number;
}

/** Where a group of notes is named: the middle of those around the
 *  square, `cell` wide, most of them lie in, so a name whose notes lie in
 *  two parts of the map is named on the bigger. With how many notes lie
 *  there and how wide they spread. */
export function spot(notes: MapNote[], cell: number) {
	const counts = new Map<string, number>();
	let [most, bestX, bestY] = [0, 0, 0];
	for (const note of notes) {
		const [i, j] = [Math.floor(note.x / cell), Math.floor(note.y / cell)];
		const count = (counts.get(`${i},${j}`) ?? 0) + 1;
		counts.set(`${i},${j}`, count);
		if (count > most) [most, bestX, bestY] = [count, i, j];
	}
	const near = notes.filter(
		(note) =>
			Math.abs(Math.floor(note.x / cell) - bestX) <= 1 &&
			Math.abs(Math.floor(note.y / cell) - bestY) <= 1
	);
	const xs = near.map((note) => note.x);
	const ys = near.map((note) => note.y);
	return {
		x: xs.reduce((sum, x) => sum + x, 0) / near.length,
		y: ys.reduce((sum, y) => sum + y, 0) / near.length,
		count: near.length,
		minY: Math.min(...ys),
		maxY: Math.max(...ys),
		wide: Math.max(Math.max(...xs) - Math.min(...xs), Math.max(...ys) - Math.min(...ys))
	};
}

/** How far apart notes usually lie on the map: the side of the square
 *  each would have, sharing out the box they span. */
export function spacing(notes: MapNote[]) {
	if (notes.length < 2) return 1;
	const xs = notes.map((note) => note.x);
	const ys = notes.map((note) => note.y);
	const area = (Math.max(...xs) - Math.min(...xs)) * (Math.max(...ys) - Math.min(...ys));
	return Math.sqrt(area / notes.length) || 1;
}

/** The first and last of the days. */
export function extentOf(days: Iterable<number>) {
	let [first, last] = [Infinity, -Infinity];
	for (const day of days) [first, last] = [Math.min(first, day), Math.max(last, day)];
	return [first, last] as const;
}

/** How many of the days each of `count` stretches of time from the first to
 *  the last holds, highest at 1, and the days it covers, for the bars above
 *  the dates. */
export function timeBars(
	days: Iterable<number>,
	[first, last]: readonly [number, number],
	count: number
) {
	const length = (last - first + 1) / count;
	const counts = Array<number>(count).fill(0);
	for (const day of days) {
		counts[Math.min(count - 1, Math.floor((day - first) / length))]++;
	}
	const most = Math.max(1, ...counts);
	return counts.map((held, i) => ({
		height: held / most,
		from: first + i * length,
		to: first + (i + 1) * length
	}));
}

/** The key the notes that mention no name are picked by, which no name has. */
export const NO_NAME = '';

/** The notes drawn in one go per colour: those in no thread first. */
export function threadGroups(
	notes: MapNote[],
	threads: Thread[],
	threadOf: ReadonlyMap<string, Thread>
) {
	const order = new Map(threads.map((thread, i) => [thread, i]));
	const loose: MapNote[] = [];
	const lists: MapNote[][] = threads.map(() => []);
	for (const note of notes) {
		const thread = threadOf.get(note.id);
		(thread ? lists[order.get(thread)!] : loose).push(note);
	}
	return [[undefined, loose] as const, ...threads.map((thread, i) => [thread, lists[i]] as const)];
}

/** Each colour's notes, those `lit` lets through apart from those it
 *  leaves out; all of them through while it is null. */
export function splitGroups(groups: ReturnType<typeof threadGroups>, lit: Set<string> | null) {
	return groups.map(([thread, notes]) => ({
		thread,
		shown: lit ? notes.filter((note) => lit.has(note.id)) : notes,
		left: lit ? notes.filter((note) => !lit.has(note.id)) : []
	}));
}

/** The notes mentioning each name, by key. */
export function notesByName(notes: MapNote[]) {
	const out = new Map<string, MapNote[]>();
	for (const note of notes) {
		for (const key of note.mentions) {
			const named = out.get(key);
			if (named) named.push(note);
			else out.set(key, [note]);
		}
	}
	return out;
}

/** Every name with its number of notes, the pinned first, as on the list
 *  of names, then the biggest. */
export function legendOf(
	byName: Map<string, MapNote[]>,
	names: ReadonlyMap<string, MentionSummary>
) {
	return [...byName]
		.map(([key, notes]) => ({ key, count: notes.length, pinned: !!names.get(key)?.pinned }))
		.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.count - a.count);
}

/** Where each name and each titled thread of more than one note is named,
 *  in squares `cell` wide. */
export function spotsOf(
	legend: { key: string }[],
	byName: Map<string, MapNote[]>,
	groups: ReturnType<typeof threadGroups>,
	cell: number
) {
	return {
		names: legend.map(({ key }) => ({ key, ...spot(byName.get(key)!, cell) })),
		threads: groups
			.filter(([thread, notes]) => thread?.title && notes.length > 1)
			.map(([thread, notes]) => ({ thread: thread!, ...spot(notes, cell) }))
	};
}
