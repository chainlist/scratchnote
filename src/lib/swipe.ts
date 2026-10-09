import { MediaQuery } from 'svelte/reactivity';

/** A phone: a narrow screen worked by touch. */
export const phone = new MediaQuery('(width < 40rem) and (pointer: coarse)');

/** How far a finger goes before it is a swipe rather than a tap. */
const SLOP = 10;

export interface Swipe {
	/** Along which the swipe goes: left and right, or up and down. */
	axis: 'x' | 'y';
	/** Whether a touch starting here at this point may become the swipe. */
	accept?: (touch: Touch, target: Element) => boolean;
	/** As the finger moves, by how far it has gone along the axis. */
	move?: (distance: number) => void;
	/** As it lifts, by how far it went and how fast, in px per ms; 0 and 0
	 *  for a touch the system took away. */
	end: (distance: number, speed: number) => void;
}

/**
 * Follow a one-finger swipe on `element`. A touch that goes the other way
 * first is left to the page, to scroll; one going this way stops the page
 * scrolling for as long as it lasts.
 */
export function onSwipe(element: HTMLElement | Document, swipe: Swipe) {
	let start: { x: number; y: number; time: number } | null = null;
	let state: 'pending' | 'active' | null = null;
	let distance = 0;

	const along = (touch: Touch) =>
		swipe.axis === 'x' ? touch.clientX - start!.x : touch.clientY - start!.y;

	function touchstart(event: Event) {
		const { touches, target } = event as TouchEvent;
		state = null;
		if (touches.length !== 1 || !(target instanceof Element)) return;
		const touch = touches[0];
		if (swipe.accept && !swipe.accept(touch, target)) return;
		start = { x: touch.clientX, y: touch.clientY, time: event.timeStamp };
		state = 'pending';
	}

	function touchmove(event: Event) {
		if (!state || !start) return;
		const { touches } = event as TouchEvent;
		const touch = touches[0];
		if (state === 'pending') {
			const dx = Math.abs(touch.clientX - start.x);
			const dy = Math.abs(touch.clientY - start.y);
			if (Math.max(dx, dy) < SLOP) return;
			const ours = swipe.axis === 'x' ? dx > dy * 1.2 : dy > dx * 1.2;
			// Too late once the page has begun scrolling.
			state = ours && event.cancelable ? 'active' : null;
			if (!state) return;
		}
		event.preventDefault();
		distance = along(touch);
		swipe.move?.(distance);
	}

	function touchend(event: Event) {
		if (state === 'active' && start) {
			const elapsed = Math.max(1, event.timeStamp - start.time);
			if (event.type === 'touchcancel') swipe.end(0, 0);
			else swipe.end(distance, distance / elapsed);
		}
		state = null;
		start = null;
		distance = 0;
	}

	element.addEventListener('touchstart', touchstart, { passive: true });
	element.addEventListener('touchmove', touchmove, { passive: false });
	element.addEventListener('touchend', touchend);
	element.addEventListener('touchcancel', touchend);
	return () => {
		element.removeEventListener('touchstart', touchstart);
		element.removeEventListener('touchmove', touchmove);
		element.removeEventListener('touchend', touchend);
		element.removeEventListener('touchcancel', touchend);
	};
}

/** Text and fields keep their own touches. */
export const inText = (target: Element) =>
	target.closest('input, textarea, select, [contenteditable="true"], .cm-editor') !== null;
