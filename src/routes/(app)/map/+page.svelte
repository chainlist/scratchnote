<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onMount, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import ScanIcon from '@lucide/svelte/icons/scan';
	import XIcon from '@lucide/svelte/icons/x';
	import {
		getNote,
		mapLinks,
		onMapChanged,
		type MapNote,
		type Note,
		type Thread
	} from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Card from '#lib/components/ui/card/index.js';
	import * as ToggleGroup from '#lib/components/ui/toggle-group/index.js';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';
	import { graph, samePlaces, type Graph, type GraphNode } from './graph.js';
	import { islands } from './islands.js';

	let { data } = $props();

	const shell = getShell();

	/** How far from a dot, in CSS pixels, the pointer still points at it. */
	const REACH = 8;
	/** A dot's radius, in CSS pixels, with every note in view. */
	const DOT = 2.5;
	/** Room left around the notes when they are fitted in view. */
	const MARGIN = 24;
	/** How close, in CSS pixels, two dots must look to be in one category. */
	const GAP = 12;
	/** How wide or tall, in CSS pixels, an island must look to be named. */
	const NAMED = 32;

	let canvas = $state<HTMLCanvasElement>();
	let width = $state(0);
	let height = $state(0);
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
	/** Every thread on the map with its number of notes, biggest first. */
	const legend = $derived(
		groups
			.filter(([thread, notes]) => thread && notes.length)
			.map(([thread, notes]) => ({ thread: thread!, count: notes.length }))
			.sort((a, b) => b.count - a.count)
	);
	const looseCount = $derived(groups[0][1].length);
	const hoveredThread = $derived(hovered ? threadOf.get(hovered.id) : undefined);
	const selectedThread = $derived(selected ? threadOf.get(selected.id) : undefined);
	/** The thread brought forward: the one pointed at, else the one clicked. */
	const focusThread = $derived(hovered ? hoveredThread : selectedThread);
	const preview = $derived(hovered ? previews.get(hovered.id) : undefined);
	const selectedPreview = $derived(selected ? previews.get(selected.id) : undefined);

	/** The zoom, in steps of √2 from every note in view, so the categories
	 *  shown change only now and then. */
	const level = $derived(Math.round(2 * Math.log2(view.scale / fitScale)));
	const categories = $derived(
		new Map(data.categories.categories.map((category) => [category.id, category]))
	);
	/** The level of the categories shown: those of notes that look `GAP` apart
	 *  or less, so zooming in parts a category into the ones it holds. Never
	 *  under level 0, nor over the last, where every note is in one. */
	const categoryLevel = $derived.by(() => {
		const { base, categories } = data.categories;
		if (!base) return 0;
		const last = categories.reduce((last, category) => Math.max(last, category.high), 0);
		const wanted = 2 * Math.log2(GAP / (fitScale * Math.SQRT2 ** level * base));
		return Math.min(last, Math.max(0, Math.round(wanted)));
	});
	/** How close notes of one category shown lie on the map. */
	const reach = $derived(data.categories.base * Math.SQRT2 ** categoryLevel);
	const shores = $derived(islands(data.notes, categories, categoryLevel));

	/** A colour per thread, the same from one launch to the next. */
	function colour(thread: Thread) {
		let hash = 0;
		for (const char of thread.id) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
		return `hsl(${hash % 360} 70% 58%)`;
	}

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
		// Behind the dots, a shape around each island: a disc around each of
		// its dots, wide enough that the discs of dots within reach meet.
		const foreground = style.getPropertyValue('--foreground').trim() || 'black';
		const graphing = mode === 'graph' ? graphed : null;
		if (!graphing) {
			const shore = (reach * scale) / 2 + 4;
			context.fillStyle = foreground;
			context.globalAlpha = 0.06;
			for (const island of shores) dots(island.notes, shore);
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
		for (const [thread, notes] of groups) {
			// Pointing at or clicking a note of a thread brings its thread forward.
			context.globalAlpha = focusThread && thread !== focusThread ? 0.25 : thread ? 0.9 : 0.55;
			context.fillStyle = thread ? colour(thread) : grey;
			dots(notes, radius);
		}
		context.globalAlpha = 1;
		if (focusThread) {
			context.fillStyle = colour(focusThread);
			dots(groups.find(([thread]) => thread === focusThread)?.[1] ?? [], radius + 1);
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
		if (graphing) {
			context.globalAlpha = 1;
			return;
		}

		// Over the dots, each island big enough on screen named above it, else
		// below, the biggest first, leaving out a name that would cover one
		// already shown.
		context.font = `500 12px ${style.fontFamily}`;
		context.textAlign = 'center';
		context.textBaseline = 'bottom';
		context.lineJoin = 'round';
		context.lineWidth = 3;
		context.strokeStyle = style.getPropertyValue('--background').trim() || 'white';
		context.fillStyle = foreground;
		context.globalAlpha = 0.8;
		const named: { x0: number; x1: number; y0: number; y1: number }[] = [];
		for (const island of [...shores].sort((a, b) => b.notes.length - a.notes.length)) {
			const [x0, x1] = [island.minX * scale + left, island.maxX * scale + left];
			const [y0, y1] = [island.minY * scale + top, island.maxY * scale + top];
			if (!island.name || Math.max(x1 - x0, y1 - y0) < NAMED) continue;
			const x = (x0 + x1) / 2;
			const half = context.measureText(island.name).width / 2 + 4;
			const box = [y0 - 4, y1 + 20]
				.map((y) => ({ x0: x - half, x1: x + half, y0: y - 16, y1: y }))
				.find(
					(box) =>
						box.x1 > 0 &&
						box.x0 < width &&
						box.y1 > 0 &&
						box.y0 < height &&
						!named.some((o) => box.x0 < o.x1 && o.x0 < box.x1 && box.y0 < o.y1 && o.y0 < box.y1)
				);
			if (!box) continue;
			named.push(box);
			context.strokeText(island.name, x, box.y1);
			context.fillText(island.name, x, box.y1);
		}
		context.globalAlpha = 1;
	}

	$effect(draw);

	/** The note whose dot is nearest the point, if near enough. */
	function nearest(x: number, y: number) {
		let best: MapNote | null = null;
		let bestDistance = REACH * REACH;
		for (const note of data.notes) {
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
		if (drag) {
			const dx = event.clientX - drag.x;
			const dy = event.clientY - drag.y;
			if (!drag.moved && Math.hypot(dx, dy) > 3) {
				drag.moved = dragging = true;
				hovered = null;
				if (drag.node) graphed?.simulation.alphaTarget(0.3).restart();
			}
			if (drag.moved && drag.node) {
				const point = at(event);
				drag.node.fx = (point.x - view.left) / view.scale;
				drag.node.fy = (point.y - view.top) / view.scale;
				return;
			}
			if (drag.moved) {
				view = { ...view, left: drag.left + dx, top: drag.top + dy };
				return;
			}
		}
		const point = at(event);
		hovered = nearest(point.x, point.y);
	}

	/** A click, not the end of a drag, shows the note under it, or none. */
	function up(event: PointerEvent) {
		if (drag && !drag.moved) {
			const point = at(event);
			selectedId = nearest(point.x, point.y)?.id ?? null;
		}
		if (drag?.node) {
			drag.node.fx = drag.node.fy = null;
			graphed?.simulation.alphaTarget(0);
		}
		drag = null;
		dragging = false;
	}

	/** The wheel zooms about the pointer. Not passive, to keep the page still. */
	function zoomable(node: HTMLCanvasElement) {
		const wheel = (event: WheelEvent) => {
			event.preventDefault();
			following = false;
			const point = at(event);
			const scale = Math.min(
				fitScale * 200,
				Math.max(fitScale / 2, view.scale * Math.exp(-event.deltaY * 0.0015))
			);
			const by = scale / view.scale;
			view = {
				scale,
				left: point.x - (point.x - view.left) * by,
				top: point.y - (point.y - view.top) * by
			};
			hovered = nearest(point.x, point.y);
		};
		node.addEventListener('wheel', wheel, { passive: false });
		return () => node.removeEventListener('wheel', wheel);
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
			untrack(() => graphed?.simulation.stop());
			return;
		}
		const notes = data.notes;
		const current = untrack(() => graphed);
		if (current && samePlaces(current.notes, notes)) {
			if (current.simulation.alpha() > current.simulation.alphaMin()) current.simulation.restart();
			return;
		}
		let live = true;
		void mapLinks().then((links) => {
			if (!live) return;
			current?.simulation.stop();
			graphed = graph(notes, links, current?.nodes, () => {
				steps++;
				if (following) fit();
			});
		});
		return () => {
			live = false;
		};
	});

	// The text of the notes pointed at or clicked, read once.
	$effect(() => {
		for (const id of [hovered?.id, selected?.id]) {
			if (!id || previews.has(id)) continue;
			previews.set(id, null);
			void getNote(id).then((note) => previews.set(id, note));
		}
	});

	onMount(() => {
		const off = onMapChanged(() => void invalidate('app:map'));
		const themes = new MutationObserver(() => theme++);
		themes.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
		return () => {
			themes.disconnect();
			graphed?.simulation.stop();
			void off.then((unlisten) => unlisten());
		};
	});
</script>

<!-- Every note of the space by meaning, notes about the same thing close
     together, coloured by thread (SPEC 6.5). -->
<View back={shell.back} title={m.map_title()} fill>
	{#if data.notes.length === 0}
		<p class="mx-auto max-w-3xl text-base text-neutral-600">{m.map_empty()}</p>
	{:else}
		<div class="relative h-full min-h-80" bind:clientWidth={width} bind:clientHeight={height}>
			<canvas
				bind:this={canvas}
				{@attach zoomable}
				class={[
					'absolute inset-0 size-full touch-none',
					dragging ? 'cursor-grabbing' : hovered ? 'cursor-pointer' : 'cursor-grab'
				]}
				aria-label={m.map_title()}
				onpointerdown={down}
				onpointermove={move}
				onpointerup={up}
				onpointerleave={() => !drag && (hovered = null)}
			></canvas>
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
			<Card.Root
				size="sm"
				class="absolute top-2 left-2 z-10 max-h-[calc(100%-1rem)] w-64 gap-2 shadow-sm"
			>
				<Card.Content class="min-h-0 overflow-y-auto">
					<ul class="space-y-1">
						{#each legend as { thread, count } (thread.id)}
							<li class="flex items-center gap-2">
								<span class="size-2.5 shrink-0 rounded-full" style:background={colour(thread)}
								></span>
								<span class="min-w-0 flex-1 truncate">{thread.title ?? m.thread_untitled()}</span>
								<span class="text-xs text-muted-foreground tabular-nums">{count}</span>
							</li>
						{/each}
						{#if looseCount}
							<li class="flex items-center gap-2">
								<span class="size-2.5 shrink-0 rounded-full bg-muted-foreground/55"></span>
								<span class="min-w-0 flex-1 truncate">{m.map_no_thread()}</span>
								<span class="text-xs text-muted-foreground tabular-nums">{looseCount}</span>
							</li>
						{/if}
					</ul>
				</Card.Content>
				<Card.Content class="text-xs text-muted-foreground">{m.map_hint()}</Card.Content>
			</Card.Root>
			{#if selected}
				{@const note = selected}
				{@const thread = selectedThread}
				<Card.Root
					size="sm"
					class="absolute top-12 right-2 z-10 max-h-[calc(100%-3.5rem)] w-80 shadow-sm"
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
					<Card.Content class="flex items-center gap-2">
						{#if thread}
							<span class="size-2.5 shrink-0 rounded-full" style:background={colour(thread)}></span>
							<span class="min-w-0 flex-1 truncate text-xs text-muted-foreground">
								{thread.title ?? m.thread_untitled()}
							</span>
						{/if}
						<Button size="sm" class="ml-auto" onclick={() => void shell.openCited(note)}>
							{m.map_open()}
						</Button>
					</Card.Content>
				</Card.Root>
			{/if}
			{#if hovered && hoveredAt && !dragging && hovered.id !== selected?.id}
				{@const thread = hoveredThread}
				<div
					class="pointer-events-none absolute z-10 w-72 rounded-md border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-md"
					style:left="{hoveredAt.x > width - 300 ? hoveredAt.x - 300 : hoveredAt.x + 14}px"
					style:top="{hoveredAt.y > height - 140 ? hoveredAt.y - 120 : hoveredAt.y + 14}px"
				>
					<p class="text-xs text-muted-foreground">{shortDay(hovered.date)}</p>
					{#if preview?.kind === 'page' && preview.subject}
						<p class="font-medium">{preview.subject}</p>
					{/if}
					{#if preview}
						<Markdown text={preview.body} links={false} class="mt-0.5 line-clamp-3" />
					{/if}
					{#if thread}
						<p class="mt-1.5 flex items-center gap-1.5 text-xs text-muted-foreground">
							<span class="size-2 shrink-0 rounded-full" style:background={colour(thread)}></span>
							<span class="truncate">{thread.title ?? m.thread_untitled()}</span>
						</p>
					{/if}
				</div>
			{/if}
		</div>
	{/if}
</View>
