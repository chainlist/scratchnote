import type { MapNote, Thread } from '#lib/api.js';
import { mentionHue } from '#lib/mentions.js';
import { threadHue } from '#lib/threads.js';
import type { Graph } from './graph.js';
import type { Point } from './map-data.js';

/** A dot's radius, in CSS pixels, with every note in view. */
const DOT = 2.5;
/** How wide or tall, in CSS pixels, a thread must look to be named. */
const NAMED = 32;
/** How strongly a note the search or the dates leave out is drawn. */
const DIM = 0.12;

/** Where a name or a thread was named, to point at it there. */
export interface Label {
	x0: number;
	x1: number;
	y0: number;
	y1: number;
	name?: string;
	thread?: Thread;
}

/** Where a group of notes is named, as `spot` finds it. */
interface Spot {
	x: number;
	minY: number;
	maxY: number;
	count: number;
	wide: number;
}

/** Everything one drawing of the map shows. */
export interface MapScene {
	width: number;
	height: number;
	/** From the map to the canvas: `x * scale + left`, `y * scale + top`. */
	view: { scale: number; left: number; top: number };
	/** The scale with every note in view. */
	fitScale: number;
	/** Where a note is drawn: its place on the map, or in the graph. */
	place: (note: MapNote) => Point;
	/** The graph, when the notes are shown as one rather than as the map. */
	graph: Graph | null;
	/** How far apart notes usually lie on the map. */
	spacing: number;
	/** Each colour's notes: those let through, and those left out. */
	parts: { thread: Thread | undefined; shown: MapNote[]; left: MapNote[] }[];
	/** The notes let through by the search, the dates and the name picked, or null for all. */
	lit: Set<string> | null;
	focusThread: Thread | undefined;
	pointedName: string | null;
	pickedName: string | null;
	/** Whether a note is of the name pointed at, else of the thread brought forward. */
	inFocus: (note: MapNote) => boolean;
	hovered: MapNote | null;
	selected: MapNote | null;
	/** Where each name and titled thread is named; read only on the map, not the graph. */
	spots: { names: (Spot & { key: string })[]; threads: (Spot & { thread: Thread })[] };
	nameOf: (key: string) => string;
	/** The root's font size, in CSS pixels. */
	rem: number;
}

/** A thread's own colour, as `.name-mark` draws it in the lists. */
export const colour = (thread: Thread, dark: boolean) =>
	`oklch(${dark ? '0.72 0.12' : '0.62 0.13'} ${threadHue(thread.id)})`;

/**
 * Draw the map: the dots in their threads' colours, the notes left out
 * faint, the threads' shapes or the graph's links behind, and on the map
 * the names and threads' titles over them. Returns where each label went.
 */
export function drawMap(context: CanvasRenderingContext2D, scene: MapScene): Label[] {
	const { width, height, view, place, parts, focusThread, pointedName, inFocus } = scene;
	context.clearRect(0, 0, width, height);

	const style = getComputedStyle(context.canvas as HTMLCanvasElement);
	const grey = style.getPropertyValue('--muted-foreground').trim() || 'gray';
	const dark = document.documentElement.classList.contains('dark');
	const { scale, left, top } = view;
	const radius = Math.min(6, DOT * Math.max(1, scale / scene.fitScale) ** 0.3);
	const dots = (notes: MapNote[], r: number) => {
		context.beginPath();
		for (const note of notes) {
			const at = place(note);
			const x = at.x * scale + left;
			const y = at.y * scale + top;
			if (x < -r || y < -r || x > width + r || y > height + r) continue;
			context.moveTo(x + r, y);
			context.arc(x, y, r, 0, 2 * Math.PI);
		}
		context.fill();
	};
	// Behind the dots, a faint shape in its colour around each thread of
	// more than one note: a disc around each of its dots, half as wide as
	// notes usually lie apart, so the discs of its notes lying close meet.
	const foreground = style.getPropertyValue('--foreground').trim() || 'black';
	const graphing = scene.graph;
	if (!graphing) {
		const halo = Math.min(40, Math.max(6, 0.5 * scene.spacing * scale));
		for (const { thread, shown } of parts) {
			if (!thread || shown.length < 2) continue;
			context.globalAlpha = focusThread && thread !== focusThread ? 0.04 : 0.1;
			context.fillStyle = colour(thread, dark);
			dots(shown, halo);
		}
	} else {
		// Behind the dots, the links: faint, those of the note pointed at
		// or clicked brighter.
		const lines = (links: Graph['links']) => {
			context.beginPath();
			for (const { source, target } of links) {
				context.moveTo(source.x * scale + left, source.y * scale + top);
				context.lineTo(target.x * scale + left, target.y * scale + top);
			}
			context.stroke();
		};
		context.strokeStyle = foreground;
		context.lineWidth = 1;
		context.globalAlpha = 0.12;
		lines(graphing.links);
		const focus = scene.hovered?.id ?? scene.selected?.id;
		if (focus) {
			context.globalAlpha = 0.6;
			lines(
				graphing.links.filter(
					({ source, target }) => source.note.id === focus || target.note.id === focus
				)
			);
		}
	}
	// The notes the search, the dates or the name picked leave out, faint,
	// under the rest.
	context.globalAlpha = DIM;
	for (const { thread, left } of parts) {
		context.fillStyle = thread ? colour(thread, dark) : grey;
		if (left.length) dots(left, radius);
	}
	// Pointing at a name or a thread, or at or clicking a note of a thread,
	// brings its notes forward.
	const focusing = pointedName !== null || !!focusThread;
	for (const { thread, shown } of parts) {
		context.globalAlpha = focusing ? 0.25 : thread ? 0.9 : 0.55;
		context.fillStyle = thread ? colour(thread, dark) : grey;
		dots(focusing ? shown.filter((note) => !inFocus(note)) : shown, radius);
	}
	context.globalAlpha = 1;
	if (focusing) {
		for (const { thread, shown } of parts) {
			context.fillStyle = thread ? colour(thread, dark) : grey;
			dots(shown.filter(inFocus), radius + 1);
		}
	}
	context.strokeStyle = foreground;
	context.lineWidth = 1.5;
	const ring = (x: number, y: number) => {
		context.beginPath();
		context.arc(x, y, radius + 3, 0, 2 * Math.PI);
		context.stroke();
	};
	for (const note of [scene.hovered, scene.selected]) {
		if (note) ring(place(note).x * scale + left, place(note).y * scale + top);
	}
	const labels: Label[] = [];
	if (graphing) return labels;

	// Over the dots, each name as a chip in its colour above the notes
	// most of its notes lie among, else below, the pinned and the biggest
	// first; then the title of each thread wide enough on screen, the same
	// way, the biggest first. A label that would cover one already shown
	// is left out.
	// In rem, as the lists around the map are: labels grow with the text size.
	const { rem, spots, pickedName, lit } = scene;
	const labelBox = (text: string, at: { x: number; minY: number; maxY: number }, pad: number) => {
		const x = at.x * scale + left;
		const half = context.measureText(text).width / 2 + pad * rem;
		return [at.minY * scale + top - 0.375 * rem, at.maxY * scale + top + 1.625 * rem]
			.map((y) => ({ x0: x - half, x1: x + half, y0: y - 1.25 * rem, y1: y }))
			.find(
				(box) =>
					box.x1 > 0 &&
					box.x0 < width &&
					box.y1 > 0 &&
					box.y0 < height &&
					!labels.some((o) => box.x0 < o.x1 && o.x0 < box.x1 && box.y0 < o.y1 && o.y0 < box.y1)
			);
	};
	context.font = `500 ${0.75 * rem}px ${style.fontFamily}`;
	const chipRadius = (parseFloat(style.getPropertyValue('--radius')) || 0) * 0.8 * rem;
	context.textAlign = 'center';
	context.textBaseline = 'middle';
	for (const name of spots.names) {
		if (name.count < 2 || (pickedName !== null && pickedName !== name.key)) continue;
		const text = scene.nameOf(name.key);
		const box = labelBox(text, name, 0.375);
		if (!box) continue;
		labels.push({ ...box, name: name.key });
		const hue = mentionHue(name.key);
		context.globalAlpha = pointedName === null || pointedName === name.key ? 1 : 0.5;
		context.fillStyle = dark ? `oklch(0.32 0.06 ${hue})` : `oklch(0.92 0.05 ${hue})`;
		context.beginPath();
		// The chips' corners follow the radius setting, as rounded-md does.
		context.roundRect(box.x0, box.y0, box.x1 - box.x0, box.y1 - box.y0, chipRadius);
		context.fill();
		context.fillStyle = dark ? `oklch(0.86 0.09 ${hue})` : `oklch(0.42 0.1 ${hue})`;
		context.fillText(text, (box.x0 + box.x1) / 2, (box.y0 + box.y1) / 2);
	}
	context.lineJoin = 'round';
	context.lineWidth = 3;
	context.strokeStyle = style.getPropertyValue('--background').trim() || 'white';
	context.fillStyle = foreground;
	for (const spotted of [...spots.threads].sort((a, b) => b.count - a.count)) {
		const { thread } = spotted;
		if (spotted.wide * scale < NAMED) continue;
		if (lit && !thread.notes.some((id) => lit.has(id))) continue;
		const title = thread.title!;
		const text = title.length > 40 ? `${title.slice(0, 39)}…` : title;
		const box = labelBox(text, spotted, 0.25);
		if (!box) continue;
		labels.push({ ...box, thread });
		context.globalAlpha = focusThread && focusThread !== thread ? 0.4 : 0.85;
		const [x, y] = [(box.x0 + box.x1) / 2, (box.y0 + box.y1) / 2];
		context.strokeText(text, x, y);
		context.fillText(text, x, y);
	}
	context.globalAlpha = 1;
	return labels;
}
