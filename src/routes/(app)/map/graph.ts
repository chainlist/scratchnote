import {
	forceLink,
	forceManyBody,
	forceSimulation,
	forceX,
	forceY,
	type Simulation,
	type SimulationLinkDatum,
	type SimulationNodeDatum
} from 'd3-force';
import type { MapLinks, MapNote } from '#lib/api.js';

/** A note in the graph, where the simulation has it. */
export interface GraphNode extends SimulationNodeDatum {
	note: MapNote;
	x: number;
	y: number;
}

export type GraphLink = SimulationLinkDatum<GraphNode> & { source: GraphNode; target: GraphNode };

export interface Graph {
	/** The notes it was made from, to tell when they are read again. */
	notes: MapNote[];
	nodes: Map<string, GraphNode>;
	links: GraphLink[];
	simulation: Simulation<GraphNode, GraphLink>;
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
/** How hard every note pushes the others away. d3's own. */
const CHARGE = -30;
/** How hard every note is pulled to the middle, so that groups linked to
 *  nothing else stay in view. */
const GRAVITY = 0.04;

/** Every note linked to its closest notes, pulled together along its links
 *  and pushed apart from the rest until it settles, calling `tick` at each
 *  step. A note starts where it was in `before`, else at its place on the
 *  map, spread so that a link is about `LENGTH` long, so it settles in a
 *  few seconds rather than from a tangle. */
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
	const nodes = new Map<string, GraphNode>(
		notes.map((note) => {
			const was = before?.get(note.id);
			const x = was?.x ?? (note.x - middle[0]) * scale;
			const y = was?.y ?? (note.y - middle[1]) * scale;
			return [note.id, { note, x, y }];
		})
	);
	const graphLinks: GraphLink[] = pairs.map(([a, b]) => ({
		source: nodes.get(a.id)!,
		target: nodes.get(b.id)!
	}));
	const simulation = forceSimulation<GraphNode, GraphLink>([...nodes.values()])
		.force('link', forceLink<GraphNode, GraphLink>(graphLinks).distance(LENGTH))
		.force('charge', forceManyBody<GraphNode>().strength(CHARGE))
		.force('x', forceX<GraphNode>(0).strength(GRAVITY))
		.force('y', forceY<GraphNode>(0).strength(GRAVITY))
		// Notes read again move only a little from where they were.
		.alpha(before ? 0.1 : 0.5)
		.on('tick', tick);
	return { notes, nodes, links: graphLinks, simulation };
}
