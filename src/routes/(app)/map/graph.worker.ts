/// <reference lib="webworker" />
// The graph's physics, off the window's own thread: on 10,000 notes a step
// takes some 36 ms, which there would hold up every click and key for the
// seconds the graph takes to settle. The page draws what it is sent.
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

/** What the page asks: notes by their place in the list it started with. */
export type GraphRequest =
	| {
			type: 'start';
			/** Where each note starts, `x` then `y`. */
			places: Float64Array;
			/** Each link as two places in the list, one after the other. */
			links: Uint32Array;
			/** How long a link settles to. */
			length: number;
			alpha: number;
	  }
	| { type: 'pause' }
	| { type: 'resume' }
	| { type: 'drag'; index: number; x: number; y: number }
	| { type: 'release'; index: number };

/** Where every note is after a step, `x` then `y`, and whether it settled. */
export interface GraphReply {
	places: Float64Array;
	settled: boolean;
}

/** How hard every note pushes the others away. d3's own. */
const CHARGE = -30;
/** How hard every note is pulled to the middle, so that groups linked to
 *  nothing else stay in view. */
const GRAVITY = 0.04;
/** The least time between two places sent, about a frame: the page cannot
 *  draw faster. The last one is always sent. */
const EVERY_MS = 16;
/** How fast the notes keep moving while one is dragged. d3's own. */
const DRAGGING = 0.3;

type Node = SimulationNodeDatum & { x: number; y: number };

declare const self: DedicatedWorkerGlobalScope;

let nodes: Node[] = [];
let simulation: Simulation<Node, SimulationLinkDatum<Node>> | null = null;
let running = false;
let timer: ReturnType<typeof setTimeout> | undefined;
let sent = 0;

function send(settled: boolean) {
	const places = new Float64Array(nodes.length * 2);
	nodes.forEach((node, i) => {
		places[2 * i] = node.x;
		places[2 * i + 1] = node.y;
	});
	self.postMessage({ places, settled } satisfies GraphReply, [places.buffer]);
	sent = performance.now();
}

/** One step, then the next after any message waiting, until it settles. */
function step() {
	timer = undefined;
	if (!simulation || !running) return;
	simulation.tick();
	const settled = simulation.alpha() < simulation.alphaMin();
	if (settled || performance.now() - sent >= EVERY_MS) send(settled);
	if (settled) running = false;
	else timer = setTimeout(step, 0);
}

function run() {
	running = true;
	timer ??= setTimeout(step, 0);
}

self.onmessage = ({ data }: MessageEvent<GraphRequest>) => {
	switch (data.type) {
		case 'start': {
			nodes = Array.from({ length: data.places.length / 2 }, (_, i) => ({
				x: data.places[2 * i],
				y: data.places[2 * i + 1]
			}));
			const links: SimulationLinkDatum<Node>[] = [];
			for (let i = 0; i < data.links.length; i += 2) {
				links.push({ source: data.links[i], target: data.links[i + 1] });
			}
			// Stepped here rather than by d3's own timer, so that a pause
			// and a drag take effect between two steps.
			simulation = forceSimulation(nodes)
				.force('link', forceLink<Node, SimulationLinkDatum<Node>>(links).distance(data.length))
				.force('charge', forceManyBody<Node>().strength(CHARGE))
				.force('x', forceX<Node>(0).strength(GRAVITY))
				.force('y', forceY<Node>(0).strength(GRAVITY))
				.alpha(data.alpha)
				.stop();
			run();
			break;
		}
		case 'pause':
			running = false;
			clearTimeout(timer);
			timer = undefined;
			break;
		case 'resume':
			if (simulation && simulation.alpha() >= simulation.alphaMin()) run();
			break;
		case 'drag': {
			const node = nodes[data.index];
			if (!node || !simulation) break;
			node.fx = data.x;
			node.fy = data.y;
			simulation.alphaTarget(DRAGGING);
			run();
			break;
		}
		case 'release': {
			const node = nodes[data.index];
			if (!node || !simulation) break;
			node.fx = node.fy = null;
			simulation.alphaTarget(0);
			run();
			break;
		}
	}
};
