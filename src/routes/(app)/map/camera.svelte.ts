import { untrack } from 'svelte';
import type { MapNote } from '#lib/api.js';
import type { Point } from './map-data.js';

/** How far from a dot, in CSS pixels, the pointer still points at it. */
const REACH = 8;
/** Room left around the notes when they are fitted in view. */
const MARGIN = 24;

/** What the camera looks at, and what a press, a drag and a click on it do. */
interface Scene<Held> {
	/** Every note on the map. */
	notes(): MapNote[];
	/** Where a note is drawn: its place on the map, or in the graph. */
	place(note: MapNote): Point;
	/** Whether a note can be pointed at, as the search and the dates let it through. */
	shown(note: MapNote): boolean;
	/** What a press at this point holds and moves instead of the view, if anything. */
	grab(point: Point): Held | null;
	/** Move what is held to this place on the map. */
	hold(held: Held, x: number, y: number): void;
	release(held: Held): void;
	/** The pointer is over this point, not dragging. */
	point(point: Point): void;
	/** A drag began, so nothing is pointed at. */
	dragStart(): void;
	/** A click, not the end of a drag. */
	click(point: Point): void;
}

/**
 * The part of the map in view, and the pointer and the wheel that move it:
 * a drag pans, or moves what `grab` holds; the wheel zooms about the
 * pointer. Made as the page starts, as it fits itself once in an effect.
 */
export class Camera<Held = never> {
	/** The canvas's size, in CSS pixels. */
	width = $state(0);
	height = $state(0);
	/** From the map to the canvas: `x * scale + left`, `y * scale + top`. */
	view = $state({ scale: 1, left: 0, top: 0 });
	/** The scale with every note in view, which zooming is measured against. */
	fitScale = 1;
	/** Whether the view keeps every note in view as the graph settles, until
	 *  the user moves it. */
	following = false;
	dragging = $state(false);

	#scene: Scene<Held>;
	#canvas: HTMLCanvasElement | undefined;
	#fitted = false;
	/** A press on the canvas: it moves the view, or what it holds. */
	#drag: {
		x: number;
		y: number;
		left: number;
		top: number;
		moved: boolean;
		held: Held | null;
	} | null = null;

	constructor(scene: Scene<Held>) {
		this.#scene = scene;
		// Fitted once there is something to show and room to show it; after
		// that the notes moving under the user's view leave it where it is.
		$effect(() => {
			if (!this.#fitted && scene.notes().length && this.width && this.height) {
				this.#fitted = true;
				untrack(() => this.fit());
			}
		});
	}

	/** Where a place on the map is on the canvas. */
	screen(at: Point): Point {
		return {
			x: at.x * this.view.scale + this.view.left,
			y: at.y * this.view.scale + this.view.top
		};
	}

	/** Every note in view, centred. */
	fit() {
		const notes = this.#scene.notes();
		const { width, height } = this;
		if (!notes.length || !width || !height) return;
		const xs = notes.map((note) => this.#scene.place(note).x);
		const ys = notes.map((note) => this.#scene.place(note).y);
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
		this.fitScale = scale;
		this.view = {
			scale,
			left: (width - (minX + maxX) * scale) / 2,
			top: (height - (minY + maxY) * scale) / 2
		};
	}

	/** Move the view so this place on the map is in its middle. */
	centre(at: Point) {
		this.following = false;
		this.view = {
			...this.view,
			left: this.width / 2 - at.x * this.view.scale,
			top: this.height / 2 - at.y * this.view.scale
		};
	}

	/** The note whose dot is nearest the point, if near enough, among those
	 *  the search and the dates let through. */
	nearest(x: number, y: number) {
		let best: MapNote | null = null;
		let bestDistance = REACH * REACH;
		for (const note of this.#scene.notes()) {
			if (!this.#scene.shown(note)) continue;
			const at = this.#scene.place(note);
			const dx = at.x * this.view.scale + this.view.left - x;
			const dy = at.y * this.view.scale + this.view.top - y;
			const distance = dx * dx + dy * dy;
			if (distance <= bestDistance) {
				bestDistance = distance;
				best = note;
			}
		}
		return best;
	}

	/** Where the pointer is on the canvas. */
	#at(event: MouseEvent): Point {
		const box = this.#canvas!.getBoundingClientRect();
		return { x: event.clientX - box.left, y: event.clientY - box.top };
	}

	down = (event: PointerEvent) => {
		if (event.button !== 0) return;
		this.#canvas!.setPointerCapture(event.pointerId);
		this.#drag = {
			x: event.clientX,
			y: event.clientY,
			left: this.view.left,
			top: this.view.top,
			moved: false,
			held: this.#scene.grab(this.#at(event))
		};
		this.following = false;
	};

	move = (event: PointerEvent) => {
		// The button let go where the canvas never heard of it.
		if (this.#drag && !(event.buttons & 1)) this.end();
		const drag = this.#drag;
		if (drag) {
			const dx = event.clientX - drag.x;
			const dy = event.clientY - drag.y;
			if (!drag.moved && Math.hypot(dx, dy) > 3) {
				drag.moved = this.dragging = true;
				this.#scene.dragStart();
			}
			if (drag.moved && drag.held !== null) {
				const point = this.#at(event);
				this.#scene.hold(
					drag.held,
					(point.x - this.view.left) / this.view.scale,
					(point.y - this.view.top) / this.view.scale
				);
				return;
			}
			if (drag.moved) {
				this.view = { ...this.view, left: drag.left + dx, top: drag.top + dy };
				return;
			}
		}
		this.#scene.point(this.#at(event));
	};

	/** A click, not the end of a drag, goes to `click`. */
	up = (event: PointerEvent) => {
		if (this.#drag && !this.#drag.moved) this.#scene.click(this.#at(event));
		this.end();
	};

	/** The press over, with no click: let go of what is held. Also when the
	 *  pointer is lost, as to Alt+Tab or a dialog mid-drag, so the map never
	 *  keeps moving under a pointer no longer pressed. */
	end = () => {
		if (this.#drag && this.#drag.held !== null) this.#scene.release(this.#drag.held);
		this.#drag = null;
		this.dragging = false;
	};

	/** Whether a press is under way, which leaving the canvas does not end. */
	get pressed() {
		return this.#drag !== null;
	}

	/** The canvas, as an attachment. The wheel zooms about the pointer. Not
	 *  passive, to keep the page still. A touchpad sends several turns
	 *  between two frames: they add up and the map zooms once a frame, so it
	 *  is drawn once rather than for each. */
	zoomable = (node: HTMLCanvasElement) => {
		this.#canvas = node;
		let turned = 0;
		let point = { x: 0, y: 0 };
		let frame = 0;
		const zoom = () => {
			frame = 0;
			const scale = Math.min(
				this.fitScale * 200,
				Math.max(this.fitScale / 2, this.view.scale * Math.exp(-turned * 0.0015))
			);
			turned = 0;
			const by = scale / this.view.scale;
			this.view = {
				scale,
				left: point.x - (point.x - this.view.left) * by,
				top: point.y - (point.y - this.view.top) * by
			};
			this.#scene.point(point);
		};
		const wheel = (event: WheelEvent) => {
			event.preventDefault();
			this.following = false;
			turned += event.deltaY;
			point = this.#at(event);
			frame ||= requestAnimationFrame(zoom);
		};
		node.addEventListener('wheel', wheel, { passive: false });
		return () => {
			node.removeEventListener('wheel', wheel);
			cancelAnimationFrame(frame);
		};
	};
}
