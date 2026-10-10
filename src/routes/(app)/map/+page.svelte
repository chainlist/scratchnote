<script lang="ts">
	import { goto, invalidate } from '$app/navigation';
	import { onMount, tick, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import ScanIcon from '@lucide/svelte/icons/scan';
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
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import View from '#lib/components/layout/View.svelte';
	import { dayNumber } from '#lib/dates.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { threadHref } from '#lib/threads.js';
	import { Camera } from './camera.svelte.js';
	import { drawMap, type Label } from './draw.js';
	import { graph, samePlaces, type Graph, type GraphNode } from './graph.js';
	import {
		extentOf,
		legendOf,
		NO_NAME,
		notesByName,
		spacing,
		splitGroups,
		spotsOf,
		threadGroups,
		timeBars
	} from './map-data.js';
	import MapDateRange from './MapDateRange.svelte';
	import MapHoverCard from './MapHoverCard.svelte';
	import MapLegend from './MapLegend.svelte';
	import MapNoteCard from './MapNoteCard.svelte';
	import MapSearch from './MapSearch.svelte';

	let { data } = $props();

	const shell = getShell();

	/** How wide, in CSS pixels, the squares are that a name or a thread is
	 *  named in the busiest of. */
	const CELL = 48;
	/** How many bars show how the notes spread over time. */
	const BARS = 40;
	/** How many found notes are listed under the search. */
	const FOUND_SHOWN = 8;

	let canvas = $state<HTMLCanvasElement>();
	/** The hover card's size, kept from one card to the next. */
	let tipSize = $state({ width: 0, height: 0 });
	/** Each note on the map by id. */
	const byId = $derived(new Map(data.notes.map((note) => [note.id, note])));
	/** The note pointed at. Kept raw, so that the pointer moving over the same
	 *  dot changes nothing and the map is not drawn again. */
	let hovered = $state.raw<MapNote | null>(null);
	/** The note clicked, shown in the card on the right. Kept by id so it
	 *  follows the map when it is read again, and goes if the note does. */
	let selectedId = $state<string | null>(null);
	const selected = $derived(selectedId ? (byId.get(selectedId) ?? null) : null);
	/** Changes with the light or dark theme, which the dots' grey follows. */
	let theme = $state(0);
	/** The notes in their places on the map, or as a graph of each note
	 *  linked to its closest notes, settling as they pull and push. */
	let mode = $state<'map' | 'graph'>('map');
	let graphed = $state.raw<Graph | null>(null);
	/** Counts the graph's steps, so each is drawn. */
	let steps = $state(0);
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
	let labels: Label[] = [];

	/** Whether a note mentions the name with this key, or none for `NO_NAME`. */
	const mentions = (note: MapNote, key: string) =>
		key === NO_NAME ? note.mentions.length === 0 : note.mentions.includes(key);
	/** Each name by key, as the newest note mentioning it types it. */
	const names = $derived(new Map(data.mentions.map((mention) => [mention.key, mention])));
	const nameOf = (key: string) => `@${names.get(key)?.name ?? key}`;

	/** Each note's day, and the first and last day of them all. */
	const days = $derived(new Map(data.notes.map((note) => [note.id, dayNumber(note.date)])));
	const extent = $derived(extentOf(days.values()));
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
	const bars = $derived(timeBars(days.values(), extent, BARS));

	const threadOf = $derived(
		new Map(data.threads.flatMap((thread) => thread.notes.map((id) => [id, thread] as const)))
	);
	const groups = $derived(threadGroups(data.notes, data.threads, threadOf));
	/** The notes on the map mentioning each name, by key, and every name on
	 *  it as the legend lists them. */
	const byName = $derived(notesByName(data.notes));
	const legend = $derived(legendOf(byName, names));
	const unnamedCount = $derived(data.notes.filter((note) => !note.mentions.length).length);
	/** Each colour's notes, those the search and the dates let through apart
	 *  from those they leave out. */
	const parts = $derived(splitGroups(groups, lit));
	const hoveredThread = $derived(hovered ? threadOf.get(hovered.id) : undefined);
	const selectedThread = $derived(selected ? threadOf.get(selected.id) : undefined);
	/** The thread brought forward: the one pointed at, its note or its name,
	 *  else the one clicked. */
	const focusThread = $derived(hovered ? hoveredThread : (pointedThread ?? selectedThread));
	/** Whether a note is of the name pointed at, else of the thread brought
	 *  forward. */
	const inFocus = (note: MapNote) =>
		pointedName !== null ? mentions(note, pointedName) : threadOf.get(note.id) === focusThread;

	/** Where a note is drawn: its place on the map, or in the graph. */
	function place(note: MapNote): { x: number; y: number } {
		return (mode === 'graph' && graphed?.nodes.get(note.id)) || note;
	}

	const camera: Camera<GraphNode> = new Camera({
		notes: () => data.notes,
		place,
		shown: (note) => !lit || lit.has(note.id),
		// In the graph, a press holds the note under it, which the notes
		// linked to it follow.
		grab: (point) => {
			if (mode !== 'graph' || !graphed) return null;
			const pressed = camera.nearest(point.x, point.y);
			return pressed ? (graphed.nodes.get(pressed.id) ?? null) : null;
		},
		hold: (node, x, y) => {
			graphed?.drag(node, x, y);
			steps++;
		},
		release: (node) => graphed?.release(node),
		point: ({ x, y }) => pointAt(x, y),
		dragStart: () => {
			hovered = pointedThread = null;
			pointedName = null;
		},
		// A click shows the note under it, or none; on a name it shows only
		// that name's notes, or every note again, and on a thread's title it
		// opens the thread.
		click: ({ x, y }) => {
			const label = labelAt(x, y);
			if (label?.name !== undefined) pickName(label.name);
			else if (label?.thread) void goto(threadHref(label.thread.id));
			else selectedId = camera.nearest(x, y)?.id ?? null;
		}
	});

	/** The zoom, in steps of √2 from every note in view, so the places names
	 *  are shown at change only now and then. */
	const level = $derived(Math.round(2 * Math.log2(camera.view.scale / camera.fitScale)));
	/** Where each name and each titled thread is named at the zoom shown. */
	const spots = $derived(
		spotsOf(legend, byName, groups, CELL / (camera.fitScale * Math.SQRT2 ** level))
	);
	const notesSpacing = $derived(spacing(data.notes));

	/** Where the dot pointed at is on the canvas, following the graph as it
	 *  settles. */
	const hoveredAt = $derived.by(() => {
		void steps;
		return hovered ? camera.screen(place(hovered)) : null;
	});

	$effect(() => {
		void theme;
		void steps;
		const { width, height } = camera;
		if (!canvas || !width || !height) return;
		const ratio = devicePixelRatio;
		if (canvas.width !== Math.round(width * ratio)) canvas.width = Math.round(width * ratio);
		if (canvas.height !== Math.round(height * ratio)) canvas.height = Math.round(height * ratio);
		const context = canvas.getContext('2d');
		if (!context) return;
		context.setTransform(ratio, 0, 0, ratio, 0, 0);
		labels = drawMap(context, {
			width,
			height,
			view: camera.view,
			fitScale: camera.fitScale,
			place,
			graph: mode === 'graph' ? graphed : null,
			spacing: notesSpacing,
			parts,
			lit,
			focusThread,
			pointedName,
			pickedName,
			inFocus,
			hovered,
			selected,
			// Only the map names them, so the graph never works them out.
			get spots() {
				return spots;
			},
			nameOf,
			rem: shell.textSize
		});
	});

	/** The name or the thread named at the point, drawn over the dots. */
	const labelAt = (x: number, y: number) =>
		labels.find((box) => x >= box.x0 && x <= box.x1 && y >= box.y0 && y <= box.y1);

	/** Point at the note, else the name or the thread named, at the point. */
	function pointAt(x: number, y: number) {
		const label = labelAt(x, y);
		hovered = label ? null : camera.nearest(x, y);
		pointedName = label?.name ?? null;
		pointedThread = label?.thread ?? null;
	}

	/** Show only the notes of the name with this key, or every note again. */
	function pickName(key: string) {
		pickedName = pickedName === key ? null : key;
	}

	/** Show the notes on the map or as a graph, all of them in view. */
	function show(next: 'map' | 'graph') {
		mode = next;
		camera.following = next === 'graph';
		camera.fit();
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
				if (camera.following) camera.fit();
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
		const note = byId.get(id);
		if (note) camera.centre(place(note));
		await tick();
		openButton?.focus();
	}

	// The text of the notes pointed at or clicked, read once.
	$effect(() => {
		for (const id of [hovered?.id, selected?.id]) {
			if (!id || untrack(() => previews.has(id))) continue;
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
<svelte:window onblur={camera.end} />

<View back={shell.back} title={m.map_title()} fill>
	{#if data.notes.length === 0}
		<p class="mx-auto max-w-3xl text-base text-meta">{m.map_empty()}</p>
	{:else}
		<div
			class="@container relative h-full"
			bind:clientWidth={camera.width}
			bind:clientHeight={camera.height}
		>
			<canvas
				bind:this={canvas}
				{@attach camera.zoomable}
				class={[
					'absolute inset-0 size-full touch-none',
					camera.dragging
						? 'cursor-grabbing'
						: hovered || pointedName !== null || pointedThread
							? 'cursor-pointer'
							: 'cursor-grab'
				]}
				aria-hidden="true"
				onpointerdown={camera.down}
				onpointermove={camera.move}
				onpointerup={camera.up}
				onpointercancel={camera.end}
				onlostpointercapture={camera.end}
				onpointerleave={() => {
					if (camera.pressed) return;
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
					onclick={() => camera.fit()}
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
					<MapSearch
						bind:query
						{found}
						count={lit?.size ?? 0}
						shown={foundShown}
						{previews}
						dayOf={(id) => byId.get(id)?.date ?? ''}
						{selectedId}
						onpick={(id) => void pickFound(id)}
					/>
					<MapDateRange {extent} {bars} bind:span />
				</Card.Content>
				<Card.Content class="min-h-0 overflow-y-auto">
					<MapLegend
						entries={[...legend, ...(unnamedCount ? [{ key: NO_NAME, count: unnamedCount }] : [])]}
						picked={pickedName}
						onpoint={(key) => (pointedName = key)}
						onpick={pickName}
						{nameOf}
					/>
				</Card.Content>
				<Card.Content class="text-xs text-muted-foreground">{m.map_hint()}</Card.Content>
			</Card.Root>
			{#if selected}
				<MapNoteCard
					bind:openButton
					note={selected}
					preview={previews.get(selected.id)}
					thread={selectedThread}
					{nameOf}
					onclose={() => (selectedId = null)}
				/>
			{/if}
			{#if hovered && hoveredAt && !camera.dragging && hovered.id !== selected?.id}
				<MapHoverCard
					bind:size={tipSize}
					note={hovered}
					preview={previews.get(hovered.id)}
					thread={hoveredThread}
					at={hoveredAt}
					width={camera.width}
					height={camera.height}
					{nameOf}
				/>
			{/if}
		</div>
	{/if}
</View>
