import type { MapLinks, MapNote } from '#lib/api.js';
import type { GraphReply, GraphRequest } from './graph.worker.js';

/** A note in the graph, where the simulation last put it. */
export interface GraphNode {
	note: MapNote;
	x: number;
	y: number;
}

export interface GraphLink {
	source: GraphNode;
	target: GraphNode;
}

export interface Graph {
	/** The notes it was made from, to tell when they are read again. */
	notes: MapNote[];
	nodes: Map<string, GraphNode>;
	links: GraphLink[];
	/** Keep the notes where they are, or let them go on settling. */
	pause(): void;
	resume(): void;
	/** Hold a note at a place, the notes linked to it following, then let it go. */
	drag(node: GraphNode, x: number, y: number): void;
	release(node: GraphNode): void;
	/** Stop it for good. */
	close(): void;
}

/** Whether two readings of the map hold the same notes in the same places,
 *  as after only the threads changed: a graph made from one goes on as it is
 *  for the other. */
export function samePlaces(a: MapNote[], b: MapNote[]) {
	return (
		a === b ||
		(a.length === b.length &&
			a.every((note, i) => note.id === b[i].id && note.x === b[i].x && note.y === b[i].y))
	);
}

/** How long a link between two notes settles to. */
const LENGTH = 30;

/** Every note linked to its closest notes, pulled together along its links
 *  and pushed apart from the rest until it settles, in a worker, calling
 *  `tick` at most once a frame with the notes' new places. A note starts
 *  where it was in `before`, else at its place on the map, spread so that a
 *  link is about `LENGTH` long, so it settles in a few seconds rather than
 *  from a tangle. */
export function graph(
	notes: MapNote[],
	links: MapLinks,
	before: Map<string, GraphNode> | undefined,
	tick: () => void
): Graph {
	const byId = new Map(notes.map((note) => [note.id, note]));
	const pairs: [MapNote, MapNote][] = [];
	for (let i = 0; i < links.links.length; i += 2) {
		const a = byId.get(links.ids[links.links[i]]);
		const b = byId.get(links.ids[links.links[i + 1]]);
		if (a && b) pairs.push([a, b]);
	}
	const lengths = pairs
		.map(([a, b]) => Math.sqrt((a.x - b.x) ** 2 + (a.y - b.y) ** 2))
		.sort((a, b) => a - b);
	const scale = LENGTH / Math.max(lengths[Math.floor(lengths.length / 2)] ?? 1, 1e-6);
	const middle = notes.reduce(
		(sum, note) => [sum[0] + note.x / notes.length, sum[1] + note.y / notes.length],
		[0, 0]
	);
	const list: GraphNode[] = notes.map((note) => {
		const was = before?.get(note.id);
		const x = was?.x ?? (note.x - middle[0]) * scale;
		const y = was?.y ?? (note.y - middle[1]) * scale;
		return { note, x, y };
	});
	const nodes = new Map(list.map((node) => [node.note.id, node]));
	const index = new Map(list.map((node, i) => [node, i]));
	const graphLinks: GraphLink[] = pairs.map(([a, b]) => ({
		source: nodes.get(a.id)!,
		target: nodes.get(b.id)!
	}));

	const places = new Float64Array(list.length * 2);
	list.forEach((node, i) => {
		places[2 * i] = node.x;
		places[2 * i + 1] = node.y;
	});
	const ends = new Uint32Array(graphLinks.length * 2);
	graphLinks.forEach((link, i) => {
		ends[2 * i] = index.get(link.source)!;
		ends[2 * i + 1] = index.get(link.target)!;
	});

	const worker = new Worker(new URL('./graph.worker.ts', import.meta.url), { type: 'module' });
	const ask = (request: GraphRequest, transfer: Transferable[] = []) =>
		worker.postMessage(request, transfer);
	let closed = false;
	// The newest places, taken in once a frame however often they come.
	let latest: Float64Array | null = null;
	let frame = 0;
	const apply = () => {
		frame = 0;
		if (closed || !latest) return;
		const at = latest;
		latest = null;
		list.forEach((node, i) => {
			node.x = at[2 * i];
			node.y = at[2 * i + 1];
		});
		tick();
	};
	worker.onmessage = ({ data }: MessageEvent<GraphReply>) => {
		if (closed) return;
		latest = data.places;
		frame ||= requestAnimationFrame(apply);
	};
	ask(
		{
			type: 'start',
			places,
			links: ends,
			length: LENGTH,
			// Notes read again move only a little from where they were.
			alpha: before ? 0.1 : 0.5
		},
		[places.buffer, ends.buffer]
	);

	return {
		notes,
		nodes,
		links: graphLinks,
		pause: () => ask({ type: 'pause' }),
		resume: () => ask({ type: 'resume' }),
		drag(node, x, y) {
			// Drawn there at once, rather than a step later.
			node.x = x;
			node.y = y;
			ask({ type: 'drag', index: index.get(node)!, x, y });
		},
		release: (node) => ask({ type: 'release', index: index.get(node)! }),
		close() {
			closed = true;
			cancelAnimationFrame(frame);
			worker.terminate();
		}
	};
}
