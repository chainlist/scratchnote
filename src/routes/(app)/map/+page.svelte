<script lang="ts">
	import { invalidate } from '$app/navigation';
	import { onMount, untrack } from 'svelte';
	import { SvelteMap } from 'svelte/reactivity';
	import ScanIcon from '@lucide/svelte/icons/scan';
	import { getNote, onMapChanged, type MapNote, type Note, type Thread } from '#lib/api.js';
	import Markdown from '#lib/components/Markdown.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import View from '#lib/components/View.svelte';
	import { shortDay } from '#lib/components/ViewHeader.svelte';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let { data } = $props();

	const shell = getShell();

	/** How far from a dot, in CSS pixels, the pointer still points at it. */
	const REACH = 8;
	/** A dot's radius, in CSS pixels, with every note in view. */
	const DOT = 2.5;
	/** Room left around the notes when they are fitted in view. */
	const MARGIN = 24;

	let canvas = $state<HTMLCanvasElement>();
	let width = $state(0);
	let height = $state(0);
	/** From the map to the canvas: `x * scale + left`, `y * scale + top`. */
	let view = $state({ scale: 1, left: 0, top: 0 });
	/** The scale with every note in view, which zooming is measured against. */
	let fitScale = 1;
	let fitted = false;
	/** The note pointed at, and where its dot is on the canvas. */
	let hovered = $state<{ note: MapNote; x: number; y: number } | null>(null);
	/** Changes with the light or dark theme, which the dots' grey follows. */
	let theme = $state(0);
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
	const hoveredThread = $derived(hovered ? threadOf.get(hovered.note.id) : undefined);
	const preview = $derived(hovered ? previews.get(hovered.note.id) : undefined);

	/** A colour per thread, the same from one launch to the next. */
	function colour(thread: Thread) {
		let hash = 0;
		for (const char of thread.id) hash = (hash * 31 + char.charCodeAt(0)) >>> 0;
		return `hsl(${hash % 360} 70% 58%)`;
	}

	/** Every note in view, centred. */
	function fit() {
		if (!data.notes.length || !width || !height) return;
		const xs = data.notes.map((note) => note.x);
		const ys = data.notes.map((note) => note.y);
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
				const x = note.x * scale + left;
				const y = note.y * scale + top;
				if (x < -r || y < -r || x > width + r || y > height + r) continue;
				context.moveTo(x + r, y);
				context.arc(x, y, r, 0, 2 * Math.PI);
			}
			context.fill();
		};
		for (const [thread, notes] of groups) {
			// Pointing at a note of a thread brings its thread forward.
			context.globalAlpha = hoveredThread && thread !== hoveredThread ? 0.25 : thread ? 0.9 : 0.55;
			context.fillStyle = thread ? colour(thread) : grey;
			dots(notes, radius);
		}
		context.globalAlpha = 1;
		if (hoveredThread) {
			context.fillStyle = colour(hoveredThread);
			dots(groups.find(([thread]) => thread === hoveredThread)?.[1] ?? [], radius + 1);
		}
		if (hovered) {
			context.strokeStyle = style.getPropertyValue('--foreground').trim() || 'black';
			context.lineWidth = 1.5;
			context.beginPath();
			context.arc(hovered.x, hovered.y, radius + 3, 0, 2 * Math.PI);
			context.stroke();
		}
	}

	$effect(draw);

	/** The note whose dot is nearest the point, if near enough. */
	function nearest(x: number, y: number) {
		let best: { note: MapNote; x: number; y: number } | null = null;
		let bestDistance = REACH * REACH;
		for (const note of data.notes) {
			const dx = note.x * view.scale + view.left - x;
			const dy = note.y * view.scale + view.top - y;
			const distance = dx * dx + dy * dy;
			if (distance <= bestDistance) {
				bestDistance = distance;
				best = { note, x: x + dx, y: y + dy };
			}
		}
		return best;
	}

	function at(event: MouseEvent) {
		const box = canvas!.getBoundingClientRect();
		return { x: event.clientX - box.left, y: event.clientY - box.top };
	}

	let drag: { x: number; y: number; left: number; top: number; moved: boolean } | null = null;
	let dragging = $state(false);

	function down(event: PointerEvent) {
		if (event.button !== 0) return;
		canvas!.setPointerCapture(event.pointerId);
		drag = { x: event.clientX, y: event.clientY, left: view.left, top: view.top, moved: false };
	}

	function move(event: PointerEvent) {
		if (drag) {
			const dx = event.clientX - drag.x;
			const dy = event.clientY - drag.y;
			if (!drag.moved && Math.hypot(dx, dy) > 3) {
				drag.moved = dragging = true;
				hovered = null;
			}
			if (drag.moved) {
				view = { ...view, left: drag.left + dx, top: drag.top + dy };
				return;
			}
		}
		const point = at(event);
		hovered = nearest(point.x, point.y);
	}

	/** A click on a dot, not the end of a drag, opens its note. */
	function up(event: PointerEvent) {
		if (drag && !drag.moved) {
			const point = at(event);
			const hit = nearest(point.x, point.y);
			if (hit) void shell.openCited(hit.note);
		}
		drag = null;
		dragging = false;
	}

	/** The wheel zooms about the pointer. Not passive, to keep the page still. */
	function zoomable(node: HTMLCanvasElement) {
		const wheel = (event: WheelEvent) => {
			event.preventDefault();
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

	// The text of the note pointed at, read once.
	$effect(() => {
		const id = hovered?.note.id;
		if (!id || previews.has(id)) return;
		previews.set(id, null);
		void getNote(id).then((note) => previews.set(id, note));
	});

	onMount(() => {
		const off = onMapChanged(() => void invalidate('app:map'));
		const themes = new MutationObserver(() => theme++);
		themes.observe(document.documentElement, { attributes: true, attributeFilter: ['class'] });
		return () => {
			themes.disconnect();
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
			<Button
				variant="outline"
				size="icon-sm"
				class="absolute top-2 right-2"
				aria-label={m.map_fit()}
				title={m.map_fit()}
				onclick={fit}
			>
				<ScanIcon />
			</Button>
			{#if hovered && !dragging}
				{@const thread = hoveredThread}
				<div
					class="pointer-events-none absolute z-10 w-72 rounded-md border bg-popover px-3 py-2 text-sm text-popover-foreground shadow-md"
					style:left="{hovered.x > width - 300 ? hovered.x - 300 : hovered.x + 14}px"
					style:top="{hovered.y > height - 140 ? hovered.y - 120 : hovered.y + 14}px"
				>
					<p class="text-xs text-muted-foreground">{shortDay(hovered.note.date)}</p>
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
