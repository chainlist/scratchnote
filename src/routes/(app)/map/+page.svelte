<script lang="ts">
	import { goto, invalidate } from '$app/navigation';
	import { onMount, tick, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import ScanIcon from '@lucide/svelte/icons/scan';
	import SearchIcon from '@lucide/svelte/icons/search';
	import XIcon from '@lucide/svelte/icons/x';
	import {
		getNote,
		getNotes,
		mapLinks,
		mapSearch,
		onMapChanged,
		stopAll,
		type MapNote,
		type Note,
		type Thread
	} from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as InputGroup from '#lib/components/ui/input-group/index.js';
	import { Slider } from '#lib/components/ui/slider/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { noteTitle } from '#lib/markdown.js';
	import { mentionHref, mentionHue } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { threadHref, threadHue } from '#lib/threads.js';
	import { graph, samePlaces, type Graph, type GraphNode } from './graph.js';

	let { data } = $props();

	const shell = getShell();

	/** How far from a dot, in CSS pixels, the pointer still points at it. */
	const REACH = 8;
	/** A dot's radius, in CSS pixels, with every note in view. */
	const DOT = 2.5;
	/** Room left around the notes when they are fitted in view. */
	const MARGIN = 24;
	/** How wide, in CSS pixels, the squares are that a name or a thread is
	 *  named in the busiest of. */
	const CELL = 48;
	/** How wide or tall, in CSS pixels, a thread must look to be named. */
	const NAMED = 32;
	/** The key the notes that mention no name are picked by, which no name has. */
	const NO_NAME = '';
	/** How strongly a note the search or the dates leave out is drawn. */
	const DIM = 0.12;
	/** How many bars show how the notes spread over time. */
	const BARS = 40;
	/** How many found notes are listed under the search. */
	const FOUND_SHOWN = 8;
	const DAY_MS = 86_400_000;

	/** A day as a number of days, which the dates shown are measured in. */
	const dayNumber = (date: string) => Math.round(Date.parse(`${date}T00:00:00Z`) / DAY_MS);
	const dateOf = (day: number) => new Date(day * DAY_MS).toISOString().slice(0, 10);

	let canvas = $state<HTMLCanvasElement>();
	let width = $state(0);
	let height = $state(0);
	/** The hover card's own size, which grows with the text size, to keep it
	 *  beside the dot and inside the map. */
	let tipWidth = $state(0);
	let tipHeight = $state(0);
	/** From the map to the canvas: `x * scale + left`, `y * scale + top`. */
	let view = $state({ scale: 1, left: 0, top: 0 });
	/** The scale with every note in view, which zooming is measured against. */
	let fitScale = 1;
	let fitted = false;
	/** The note pointed at. Kept raw, so that the pointer moving over the same
	 *  dot changes nothing and the map is not drawn again. */
	let hovered = $state.raw<MapNote | null>(null);
	/** The note clicked, shown in the card on the right. Kept by id so it
	 *  follows the map when it is read again, and goes if the note does. */
	let selectedId = $state<string | null>(null);
	const selected = $derived(
		selectedId ? (data.notes.find((note) => note.id === selectedId) ?? null) : null
	);
	/** Changes with the light or dark theme, which the dots' grey follows. */
	let theme = $state(0);
	/** The notes in their places on the map, or as a graph of each note
	 *  linked to its closest notes, settling as they pull and push. */
	let mode = $state<'map' | 'graph'>('map');
	let graphed = $state.raw<Graph | null>(null);
	/** Counts the graph's steps, so each is drawn. */
	let steps = $state(0);
	/** Whether the view keeps every note in view as the graph settles, until
	 *  the user moves it. */
	let following = false;
	const previews = new SvelteMap<string, Note | null>();
	/** What the search box holds, and the notes whose text holds its words,
	 *  null while it is empty. */
	let query = $state('');
	let found = $state.raw<Set<string> | null>(null);
	/** The first and last day shown, null while every day is. */
	let span = $state<[number, number] | null>(null);
	/** The name whose notes alone are drawn as ever, by key, `NO_NAME` for
	 *  the notes that mention none; null while every note is. */
	let pickedName = $state<string | null>(null);
	/** The name and the thread pointed at on the list or by their names on
	 *  the map, whose notes are brought forward. */
	let pointedName = $state<string | null>(null);
	let pointedThread = $state.raw<Thread | null>(null);
	/** Where each name and thread was named when the map was last drawn, to
	 *  point at it there. */
	let labels: { x0: number; x1: number; y0: number; y1: number; name?: string; thread?: Thread }[] =
		[];

	/** Whether a note mentions the name with this key, or none for `NO_NAME`. */
	const mentions = (note: MapNote, key: string) =>
		key === NO_NAME ? note.mentions.length === 0 : note.mentions.includes(key);
	/** Each name by key, as the newest note mentioning it types it. */
	const names = $derived(new Map(data.mentions.map((mention) => [mention.key, mention])));
	const nameOf = (key: string) => `@${names.get(key)?.name ?? key}`;

	/** Each note's day, and the first and last day of them all. */
	const days = $derived(new Map(data.notes.map((note) => [note.id, dayNumber(note.date)])));
	const extent = $derived.by(() => {
		let [first, last] = [Infinity, -Infinity];
		for (const day of days.values()) [first, last] = [Math.min(first, day), Math.max(last, day)];
		return [first, last] as const;
	});
	/** The notes the search, the dates and the name picked let through,
	 *  drawn as ever; null while none narrows anything. */
	const lit = $derived.by(() => {
		if (!found && !span && pickedName === null) return null;
		const through = (note: MapNote) => {
			const day = days.get(note.id)!;
			return (
				(!found || found.has(note.id)) &&
				(!span || (day >= span[0] && day <= span[1])) &&
				(pickedName === null || mentions(note, pickedName))
			);
		};
		return new Set(data.notes.filter(through).map((note) => note.id));
	});
	/** The first notes the search finds and the rest lets through, as buttons
	 *  under it: the keyboard's way to a note, as the canvas is the pointer's. */
	const foundShown = $derived(
		found && lit ? [...found].filter((id) => lit.has(id)).slice(0, FOUND_SHOWN) : []
	);
	/** How many notes each stretch of time holds, highest at 1, and the days
	 *  it covers, for the bars above the dates. */
	const bars = $derived.by(() => {
		const [first, last] = extent;
		const length = (last - first + 1) / BARS;
		const counts = Array<number>(BARS).fill(0);
		for (const day of days.values()) {
			counts[Math.min(BARS - 1, Math.floor((day - first) / length))]++;
		}
		const most = Math.max(1, ...counts);
		return counts.map((count, i) => ({
			height: count / most,
			from: first + i * length,
			to: first + (i + 1) * length
		}));
	});

	const threadOf = $derived(
		new Map(data.threads.flatMap((thread) => thread.notes.map((id) => [id, thread] as const)))
	);
	/** The notes drawn in one go per colour: those in no thread first. */
	const groups = $derived.by(() => {
		const order = new Map(data.threads.map((thread, i) => [thread, i]));
		const loose: MapNote[] = [];
		const lists: MapNote[][] = data.threads.map(() => []);
		for (const note of data.notes) {
			const thread = threadOf.get(note.id);
			(thread ? lists[order.get(thread)!] : loose).push(note);
		}
		return [
			[undefined, loose] as const,
			...data.threads.map((thread, i) => [thread, lists[i]] as const)
		];
	});
	/** The notes on the map mentioning each name, by key. */
	const byName = $derived.by(() => {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- made anew with the notes, never changed after
		const out = new Map<string, MapNote[]>();
		for (const note of data.notes) {
			for (const key of note.mentions) {
				const notes = out.get(key);
				if (notes) notes.push(note);
				else out.set(key, [note]);
			}
		}
		return out;
	});
	/** Every name on the map with its number of notes, the pinned first, as
	 *  on the list of names, then the biggest. */
	const legend = $derived(
		[...byName]
			.map(([key, notes]) => ({ key, count: notes.length, pinned: !!names.get(key)?.pinned }))
			.sort((a, b) => Number(b.pinned) - Number(a.pinned) || b.count - a.count)
	);
	const unnamedCount = $derived(data.notes.filter((note) => !note.mentions.length).length);
	/** Each colour's notes, those the search and the dates let through apart
	 *  from those they leave out. */
	const parts = $derived(
		groups.map(([thread, notes]) => ({
			thread,
			shown: lit ? notes.filter((note) => lit.has(note.id)) : notes,
			left: lit ? notes.filter((note) => !lit.has(note.id)) : []
		}))
	);
	const hoveredThread = $derived(hovered ? threadOf.get(hovered.id) : undefined);
	const selectedThread = $derived(selected ? threadOf.get(selected.id) : undefined);
	/** The thread brought forward: the one pointed at, its note or its name,
	 *  else the one clicked. */
	const focusThread = $derived(hovered ? hoveredThread : (pointedThread ?? selectedThread));
	/** Whether a note is of the name pointed at, else of the thread brought
	 *  forward. */
	const inFocus = (note: MapNote) =>
		pointedName !== null ? mentions(note, pointedName) : threadOf.get(note.id) === focusThread;
	const preview = $derived(hovered ? previews.get(hovered.id) : undefined);
	const selectedPreview = $derived(selected ? previews.get(selected.id) : undefined);

	/** The zoom, in steps of √2 from every note in view, so the places names
	 *  are shown at change only now and then. */
	const level = $derived(Math.round(2 * Math.log2(view.scale / fitScale)));
	/** How far apart notes usually lie on the map: the side of the square
	 *  each would have, sharing out the box they span. */
	const spacing = $derived.by(() => {
		if (data.notes.length < 2) return 1;
		const xs = data.notes.map((note) => note.x);
		const ys = data.notes.map((note) => note.y);
		const area = (Math.max(...xs) - Math.min(...xs)) * (Math.max(...ys) - Math.min(...ys));
		return Math.sqrt(area / data.notes.length) || 1;
	});

	/** Where a group of notes is named: the middle of those around the
	 *  square, `cell` wide, most of them lie in, so a name whose notes lie in
	 *  two parts of the map is named on the bigger. With how many notes lie
	 *  there and how wide they spread. */
	function spot(notes: MapNote[], cell: number) {
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- a count, nothing is drawn from it
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
	/** Where each name and each titled thread is named at the zoom shown. */
	const spots = $derived.by(() => {
		const cell = CELL / (fitScale * Math.SQRT2 ** level);
		return {
			names: legend.map(({ key }) => ({ key, ...spot(byName.get(key)!, cell) })),
			threads: groups
				.filter(([thread, notes]) => thread?.title && notes.length > 1)
				.map(([thread, notes]) => ({ thread: thread!, ...spot(notes, cell) }))
		};
	});

	/** A thread's own colour, as `.name-mark` draws it in the lists. */
	const colour = (thread: Thread, dark: boolean) =>
		`oklch(${dark ? '0.72 0.12' : '0.62 0.13'} ${threadHue(thread.id)})`;

	/** Where a note is drawn: its place on the map, or in the graph. */
	function place(note: MapNote): { x: number; y: number } {
		return (mode === 'graph' && graphed?.nodes.get(note.id)) || note;
	}

	/** Where the dot pointed at is on the canvas, following the graph as it
	 *  settles. */
	const hoveredAt = $derived.by(() => {
		void steps;
		if (!hovered) return null;
		const at = place(hovered);
		return { x: at.x * view.scale + view.left, y: at.y * view.scale + view.top };
	});

	/** Every note in view, centred. */
	function fit() {
		if (!data.notes.length || !width || !height) return;
		const xs = data.notes.map((note) => place(note).x);
		const ys = data.notes.map((note) => place(note).y);
		const [minX, maxX, minY, maxY] = [
			Math.min(...xs),
			Math.max(...xs),
			Math.min(...ys),
			Math.max(...ys)
		];
		const scale = Math.min(
			(width - 2 * MARGIN) / Math.max(maxX - minX, 1e-6),
			(height - 2 * MARGIN) / Math.max(maxY - minY, 1e-6)
		);
		fitScale = scale;
		view = {
			scale,
			left: (width - (minX + maxX) * scale) / 2,
			top: (height - (minY + maxY) * scale) / 2
		};
	}

	// Fitted once there is something to show and room to show it; after
	// that the notes moving under the user's view leave it where it is.
	$effect(() => {
		if (!fitted && data.notes.length && width && height) {
			fitted = true;
			untrack(fit);
		}
	});

	function draw() {
		void theme;
		void steps;
		if (!canvas || !width || !height) return;
		const ratio = devicePixelRatio;
		if (canvas.width !== Math.round(width * ratio)) canvas.width = Math.round(width * ratio);
		if (canvas.height !== Math.round(height * ratio)) canvas.height = Math.round(height * ratio);
		const context = canvas.getContext('2d');
		if (!context) return;
		context.setTransform(ratio, 0, 0, ratio, 0, 0);
		context.clearRect(0, 0, width, height);

		const style = getComputedStyle(canvas);
		const grey = style.getPropertyValue('--muted-foreground').trim() || 'gray';
		const dark = document.documentElement.classList.contains('dark');
		const { scale, left, top } = view;
		const radius = Math.min(6, DOT * Math.max(1, scale / fitScale) ** 0.3);
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
		const graphing = mode === 'graph' ? graphed : null;
		if (!graphing) {
			const halo = Math.min(40, Math.max(6, 0.5 * spacing * scale));
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
			const focus = hovered?.id ?? selected?.id;
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
		for (const note of [hovered, selected]) {
			if (note) ring(place(note).x * scale + left, place(note).y * scale + top);
		}
		labels = [];
		if (graphing) return;

		// Over the dots, each name as a chip in its colour above the notes
		// most of its notes lie among, else below, the pinned and the biggest
		// first; then the title of each thread wide enough on screen, the same
		// way, the biggest first. A label that would cover one already shown
		// is left out.
		// In rem, as the lists around the map are: labels grow with the text size.
		const rem = shell.textSize;
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
			const text = nameOf(name.key);
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
	}

	$effect(draw);

	/** The note whose dot is nearest the point, if near enough, among those
	 *  the search and the dates let through. */
	function nearest(x: number, y: number) {
		let best: MapNote | null = null;
		let bestDistance = REACH * REACH;
		for (const note of data.notes) {
			if (lit && !lit.has(note.id)) continue;
			const at = place(note);
			const dx = at.x * view.scale + view.left - x;
			const dy = at.y * view.scale + view.top - y;
			const distance = dx * dx + dy * dy;
			if (distance <= bestDistance) {
				bestDistance = distance;
				best = note;
			}
		}
		return best;
	}

	function at(event: MouseEvent) {
		const box = canvas!.getBoundingClientRect();
		return { x: event.clientX - box.left, y: event.clientY - box.top };
	}

	/** The name or the thread named at the point, drawn over the dots. */
	const labelAt = (x: number, y: number) =>
		labels.find((box) => x >= box.x0 && x <= box.x1 && y >= box.y0 && y <= box.y1);

	/** Point at the note, else the name or the thread named, at the point. */
	function pointAt(x: number, y: number) {
		const label = labelAt(x, y);
		hovered = label ? null : nearest(x, y);
		pointedName = label?.name ?? null;
		pointedThread = label?.thread ?? null;
	}

	/** A press on the canvas: it moves the view, or in the graph the note
	 *  pressed on, which the notes linked to it follow. */
	let drag: {
		x: number;
		y: number;
		left: number;
		top: number;
		moved: boolean;
		node: GraphNode | null;
	} | null = null;
	let dragging = $state(false);

	function down(event: PointerEvent) {
		if (event.button !== 0) return;
		canvas!.setPointerCapture(event.pointerId);
		const point = at(event);
		const pressed = mode === 'graph' && graphed ? nearest(point.x, point.y) : null;
		drag = {
			x: event.clientX,
			y: event.clientY,
			left: view.left,
			top: view.top,
			moved: false,
			node: pressed ? (graphed?.nodes.get(pressed.id) ?? null) : null
		};
		following = false;
	}

	function move(event: PointerEvent) {
		// The button let go where the canvas never heard of it.
		if (drag && !(event.buttons & 1)) end();
		if (drag) {
			const dx = event.clientX - drag.x;
			const dy = event.clientY - drag.y;
			if (!drag.moved && Math.hypot(dx, dy) > 3) {
				drag.moved = dragging = true;
				hovered = pointedThread = null;
				pointedName = null;
			}
			if (drag.moved && drag.node) {
				const point = at(event);
				graphed?.drag(
					drag.node,
					(point.x - view.left) / view.scale,
					(point.y - view.top) / view.scale
				);
				steps++;
				return;
			}
			if (drag.moved) {
				view = { ...view, left: drag.left + dx, top: drag.top + dy };
				return;
			}
		}
		const { x, y } = at(event);
		pointAt(x, y);
	}

	/** A click, not the end of a drag, shows the note under it, or none; on
	 *  a name it shows only that name's notes, or every note again, and on a
	 *  thread's title it opens the thread. */
	function up(event: PointerEvent) {
		if (drag && !drag.moved) {
			const { x, y } = at(event);
			const label = labelAt(x, y);
			if (label?.name !== undefined) pickName(label.name);
			else if (label?.thread) void goto(threadHref(label.thread.id));
			else selectedId = nearest(x, y)?.id ?? null;
		}
		end();
	}

	/** The press over, with no click: let go of the note held in the graph.
	 *  Also when the pointer is lost, as to Alt+Tab or a dialog mid-drag,
	 *  so the map never keeps moving under a pointer no longer pressed. */
	function end() {
		if (drag?.node) graphed?.release(drag.node);
		drag = null;
		dragging = false;
	}

	/** The wheel zooms about the pointer. Not passive, to keep the page still.
	 *  A touchpad sends several turns between two frames: they add up and
	 *  the map zooms once a frame, so it is drawn once rather than for each. */
	function zoomable(node: HTMLCanvasElement) {
		let turned = 0;
		let point = { x: 0, y: 0 };
		let frame = 0;
		const zoom = () => {
			frame = 0;
			const scale = Math.min(
				fitScale * 200,
				Math.max(fitScale / 2, view.scale * Math.exp(-turned * 0.0015))
			);
			turned = 0;
			const by = scale / view.scale;
			view = {
				scale,
				left: point.x - (point.x - view.left) * by,
				top: point.y - (point.y - view.top) * by
			};
			pointAt(point.x, point.y);
		};
		const wheel = (event: WheelEvent) => {
			event.preventDefault();
			following = false;
			turned += event.deltaY;
			point = at(event);
			frame ||= requestAnimationFrame(zoom);
		};
		node.addEventListener('wheel', wheel, { passive: false });
		return () => {
			node.removeEventListener('wheel', wheel);
			cancelAnimationFrame(frame);
		};
	}

	/** Show only the notes of the name with this key, or every note again. */
	function pickName(key: string) {
		pickedName = pickedName === key ? null : key;
	}

	/** Show the notes on the map or as a graph, all of them in view. */
	function show(next: 'map' | 'graph') {
		mode = next;
		following = next === 'graph';
		fit();
	}

	// The graph, made the first time it is shown and again from where its
	// notes were whenever they move on the map, not when only the threads
	// changed. It only runs while shown.
	$effect(() => {
		if (mode !== 'graph') {
			untrack(() => graphed?.pause());
			return;
		}
		const notes = data.notes;
		const current = untrack(() => graphed);
		if (current && samePlaces(current.notes, notes)) {
			current.resume();
			return;
		}
		let live = true;
		void mapLinks().then((links) => {
			if (!live) return;
			current?.close();
			graphed = graph(notes, links, current?.nodes, () => {
				steps++;
				if (following) fit();
			});
		});
		return () => {
			live = false;
		};
	});

	// The notes the search box finds, a moment after typing stops, and again
	// whenever the map is read again. Only the latest answer is kept.
	let searchRun = 0;
	$effect(() => {
		const words = query.trim();
		void data.notes;
		const run = ++searchRun;
		if (!words) {
			found = null;
			return;
		}
		const timer = setTimeout(() => {
			mapSearch(words)
				.then((ids) => {
					if (run === searchRun) found = new Set(ids);
				})
				.catch((e) => {
					if (run === searchRun) shell.fail(m.error_search(), e);
				});
		}, 200);
		return () => clearTimeout(timer);
	});

	// The text of the notes listed under the search, read together, once.
	$effect(() => {
		const missing = foundShown.filter((id) => !untrack(() => previews.has(id)));
		if (!missing.length) return;
		for (const id of missing) previews.set(id, null);
		void getNotes(missing).then((notes) => {
			for (const note of notes) previews.set(note.id, note);
		});
	});

	/** The Open button of the note shown, which a note picked from the list focuses. */
	let openButton = $state<HTMLElement | null>(null);

	/** A note picked from the list: the map moves to it, and its card opens
	 *  with Open in focus, so Enter again opens it. */
	async function pickFound(id: string) {
		selectedId = id;
		const note = data.notes.find((other) => other.id === id);
		if (note) {
			const at = place(note);
			following = false;
			view = {
				...view,
				left: width / 2 - at.x * view.scale,
				top: height / 2 - at.y * view.scale
			};
		}
		await tick();
		openButton?.focus();
	}

	// The text of the notes pointed at or clicked, read once.
	$effect(() => {
		for (const id of [hovered?.id, selected?.id]) {
			if (!id || previews.has(id)) continue;
			previews.set(id, null);
			void getNote(id).then((note) => previews.set(id, note));
		}
	});

	onMount(() => {
		const off = stopAll(onMapChanged(() => void invalidate('app:map')));
		const themes = new MutationObserver(() => theme++);
		// The theme is a class on the root; the accent, the font and the size are its style.
		themes.observe(document.documentElement, {
			attributes: true,
			attributeFilter: ['class', 'style']
		});
		return () => {
			themes.disconnect();
			graphed?.close();
			off();
		};
	});
</script>

<!-- Every note of the space by meaning, notes about the same thing close
     together, coloured by thread, with the names they mention (SPEC 6.5). -->
<svelte:window onblur={end} />

<View back={shell.back} title={m.map_title()} fill>
	{#if data.notes.length === 0}
		<p class="mx-auto max-w-3xl text-base text-meta">{m.map_empty()}</p>
	{:else}
		<div class="@container relative h-full" bind:clientWidth={width} bind:clientHeight={height}>
			<canvas
				bind:this={canvas}
				{@attach zoomable}
				class={[
					'absolute inset-0 size-full touch-none',
					dragging
						? 'cursor-grabbing'
						: hovered || pointedName !== null || pointedThread
							? 'cursor-pointer'
							: 'cursor-grab'
				]}
				aria-hidden="true"
				onpointerdown={down}
				onpointermove={move}
				onpointerup={up}
				onpointercancel={end}
				onlostpointercapture={end}
				onpointerleave={() => {
					if (drag) return;
					hovered = pointedThread = null;
					pointedName = null;
				}}
			></canvas>
			<!-- The map is a picture for the pointer; the keyboard and a screen
			     reader reach its notes through the search beside it. -->
			<p class="sr-only">{m.map_canvas({ count: data.notes.length })}</p>
			<div class="absolute top-2 right-2 flex items-center gap-2">
				<ToggleGroup.Root
					type="single"
					variant="outline"
					size="sm"
					value={mode}
					onValueChange={(value) => value && show(value as 'map' | 'graph')}
				>
					<ToggleGroup.Item value="map">{m.map_title()}</ToggleGroup.Item>
					<ToggleGroup.Item value="graph">{m.map_graph()}</ToggleGroup.Item>
				</ToggleGroup.Root>
				<Button
					variant="outline"
					size="icon-sm"
					aria-label={m.map_fit()}
					title={m.map_fit()}
					onclick={fit}
				>
					<ScanIcon />
				</Button>
			</div>
			<!-- In a narrow map the names go under the buttons, and a note clicked
			     opens along the bottom rather than over them. -->
			<Card.Root
				size="sm"
				class={[
					'absolute top-2 left-2 z-10 max-h-[calc(100%-1rem)] w-64 max-w-[calc(100%-1rem)] gap-2 @max-[38rem]:top-12 @max-[38rem]:max-h-[calc(100%-3.5rem)]',
					selected && '@max-[38rem]:max-h-[calc(55%-3.5rem)]'
				]}
			>
				<Card.Content class="flex flex-col gap-3">
					<div class="flex flex-col gap-1">
						<InputGroup.Root class="h-8">
							<InputGroup.Addon><SearchIcon /></InputGroup.Addon>
							<InputGroup.Input
								bind:value={query}
								placeholder={m.map_find()}
								aria-label={m.map_find()}
								onkeydown={(event) => {
									if (event.key === 'Escape' && query) {
										event.preventDefault();
										query = '';
									} else if (event.key === 'Enter' && foundShown[0]) {
										event.preventDefault();
										void pickFound(foundShown[0]);
									}
								}}
							/>
						</InputGroup.Root>
						<!-- Kept in the page while empty, so its first count is read out too. -->
						<p class="px-1 text-xs text-muted-foreground empty:sr-only" aria-live="polite">
							{#if found}
								{lit?.size ? m.page_results({ count: lit.size }) : m.page_no_match()}
							{/if}
						</p>
						{#if foundShown.length}
							<!-- The first notes found, each opening its card on the map. -->
							<ul class="-mx-1 flex flex-col" aria-label={m.map_found()}>
								{#each foundShown as id (id)}
									{@const note = previews.get(id)}
									<li>
										<button
											type="button"
											onclick={() => void pickFound(id)}
											aria-current={selectedId === id ? 'true' : undefined}
											class={[
												'flex w-full min-w-0 items-baseline gap-2 rounded-md px-1 py-1 text-left text-sm transition-colors outline-none hover:bg-muted focus-visible:ring-[3px] focus-visible:ring-ring/50 focus-visible:outline-1 focus-visible:outline-ring focus-visible:outline-solid',
												selectedId === id && 'bg-muted'
											]}
										>
											<span class="shrink-0 text-xs text-muted-foreground tabular-nums">
												{shortDay(data.notes.find((other) => other.id === id)?.date ?? '')}
											</span>
											<span class="min-w-0 flex-1 truncate">{note ? noteTitle(note) : ''}</span>
										</button>
									</li>
								{/each}
							</ul>
						{/if}
					</div>
					{#if extent[1] > extent[0]}
						{@const [from, to] = span ?? extent}
						<div class="flex flex-col gap-1.5" role="group" aria-label={m.map_dates()}>
							<div class="flex h-6 items-end gap-px" aria-hidden="true">
								{#each bars as bar, i (i)}
									<div
										class={[
											'flex-1 rounded-[1px]',
											bar.to > from && bar.from <= to ? 'bg-primary/60' : 'bg-muted-foreground/20'
										]}
										style:height="{bar.height ? Math.max(8, bar.height * 100) : 0}%"
									></div>
								{/each}
							</div>
							<Slider
								type="multiple"
								min={extent[0]}
								max={extent[1]}
								step={1}
								value={[from, to]}
								onValueChange={([first, last]) =>
									(span = first <= extent[0] && last >= extent[1] ? null : [first, last])}
								thumbLabel={(index) => (index === 0 ? m.map_dates_from() : m.map_dates_to())}
								valueText={(day) => shortDay(dateOf(day))}
							/>
							<div class="flex h-5 items-center justify-between text-xs text-muted-foreground">
								<span>{shortDay(dateOf(from))}</span>
								{#if span}
									<Button
										variant="ghost"
										size="icon-xs"
										aria-label={m.map_dates_all()}
										title={m.map_dates_all()}
										onclick={() => (span = null)}
									>
										<XIcon />
									</Button>
								{/if}
								<span>{shortDay(dateOf(to))}</span>
							</div>
						</div>
					{/if}
				</Card.Content>
				<Card.Content class="min-h-0 overflow-y-auto">
					<!-- Pointing at a name brings its notes forward; clicking it shows
					     only them, until it is clicked again. -->
					<ul class="-mx-1 space-y-px">
						{#each [...legend, ...(unnamedCount ? [{ key: NO_NAME, count: unnamedCount }] : [])] as { key, count } (key)}
							<li>
								<button
									type="button"
									class={[
										'flex w-full items-center gap-2 rounded-md px-1 py-0.5 text-left transition-colors hover:bg-muted',
										pickedName === key && 'bg-muted'
									]}
									style:--hue={key === NO_NAME ? undefined : mentionHue(key)}
									aria-pressed={pickedName === key}
									onclick={() => pickName(key)}
									onpointerenter={() => (pointedName = key)}
									onpointerleave={() => (pointedName = null)}
									onfocus={() => (pointedName = key)}
									onblur={() => (pointedName = null)}
								>
									{#if key === NO_NAME}
										<span class="min-w-0 flex-1 truncate text-muted-foreground"
											>{m.map_no_name()}</span
										>
									{:else}
										<span class="min-w-0 flex-1 truncate">
											<span class="name-tint rounded-md px-1 text-sm font-medium"
												>{nameOf(key)}</span
											>
										</span>
									{/if}
									<span class="text-xs text-muted-foreground tabular-nums">{count}</span>
								</button>
							</li>
						{/each}
					</ul>
				</Card.Content>
				<Card.Content class="text-xs text-muted-foreground">{m.map_hint()}</Card.Content>
			</Card.Root>
			{#if selected}
				{@const note = selected}
				{@const thread = selectedThread}
				<Card.Root
					size="sm"
					class="absolute top-12 right-2 z-10 max-h-[calc(100%-3.5rem)] w-80 @max-[38rem]:top-auto @max-[38rem]:bottom-2 @max-[38rem]:left-2 @max-[38rem]:max-h-[calc(45%-1rem)] @max-[38rem]:w-auto"
				>
					<Card.Header>
						<Card.Description class="text-xs">{shortDay(note.date)}</Card.Description>
						{#if selectedPreview?.kind === 'page' && selectedPreview.subject}
							<Card.Title>{selectedPreview.subject}</Card.Title>
						{/if}
						<Card.Action>
							<Button
								variant="ghost"
								size="icon-xs"
								aria-label={m.common_close()}
								title={m.common_close()}
								onclick={() => (selectedId = null)}
							>
								<XIcon />
							</Button>
						</Card.Action>
					</Card.Header>
					{#if selectedPreview}
						<Card.Content class="min-h-0 overflow-y-auto">
							<Markdown text={selectedPreview.body} links={false} />
						</Card.Content>
					{/if}
					{#if note.mentions.length}
						<Card.Content class="flex flex-wrap gap-1">
							{#each note.mentions as key (key)}
								<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- mentionHref resolves it -->
								<a
									href={mentionHref(key)}
									class="name-tint rounded-md px-1 text-sm font-medium hover:underline"
									style:--hue={mentionHue(key)}>{nameOf(key)}</a
								>
							{/each}
						</Card.Content>
					{/if}
					<Card.Content class="flex items-center gap-2">
						{#if thread}
							<span
								class="name-mark size-2.5 shrink-0 rounded-full"
								style:--hue={threadHue(thread.id)}
							></span>
							<!-- eslint-disable-next-line svelte/no-navigation-without-resolve -- threadHref resolves it -->
							<a
								href={threadHref(thread.id)}
								class="min-w-0 flex-1 truncate text-xs text-muted-foreground hover:underline"
							>
								{shell.nameOf(thread)}
							</a>
						{/if}
						<Button
							bind:ref={openButton}
							size="sm"
							class="ml-auto"
							onclick={() => void shell.openCited(note)}
						>
							{m.map_open()}
						</Button>
					</Card.Content>
				</Card.Root>
			{/if}
			{#if hovered && hoveredAt && !dragging && hovered.id !== selected?.id}
				{@const thread = hoveredThread}
				<!-- Below and right of the dot, or on whichever side has the room. -->
				<div
					bind:offsetWidth={tipWidth}
					bind:offsetHeight={tipHeight}
					class="pointer-events-none absolute z-10 w-72 max-w-[calc(100%-1rem)] rounded-md border bg-popover px-3 py-2 text-sm text-popover-foreground"
					style:left="{Math.max(
						0,
						hoveredAt.x + 14 + tipWidth > width ? hoveredAt.x - 14 - tipWidth : hoveredAt.x + 14
					)}px"
					style:top="{Math.max(
						0,
						hoveredAt.y + 14 + tipHeight > height ? hoveredAt.y - 14 - tipHeight : hoveredAt.y + 14
					)}px"
				>
					<p class="text-xs text-muted-foreground">{shortDay(hovered.date)}</p>
					{#if preview?.kind === 'page' && preview.subject}
						<p class="font-medium">{preview.subject}</p>
					{/if}
					{#if preview}
						<Markdown text={preview.body} links={false} class="mt-0.5 line-clamp-3" />
					{/if}
					{#if hovered.mentions.length}
						<p class="mt-1.5 flex flex-wrap gap-1">
							{#each hovered.mentions as key (key)}
								<span
									class="name-tint rounded-md px-1 text-xs font-medium"
									style:--hue={mentionHue(key)}>{nameOf(key)}</span
								>
							{/each}
						</p>
					{/if}
					{#if thread}
						<p class="mt-1.5 flex items-center gap-1.5 text-xs text-muted-foreground">
							<span
								class="name-mark size-2 shrink-0 rounded-full"
								style:--hue={threadHue(thread.id)}
							></span>
							<span class="truncate">{shell.nameOf(thread)}</span>
						</p>
					{/if}
				</div>
			{/if}
		</div>
	{/if}
</View>
