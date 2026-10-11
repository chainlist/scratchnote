import type { Attachment } from 'svelte/attachments';
import { quintOut } from 'svelte/easing';
import { fade, type TransitionConfig } from 'svelte/transition';

/** The easing of what arrives, as `page-in` has it in layout.css:
 *  `cubic-bezier(0.22, 1, 0.36, 1)`, quick and then settling. */
export const settle = quintOut;

/** Whether the user asked for less motion, in the settings or the system's
 *  (applyAppearance marks the root): then nothing moves across the screen,
 *  and what comes or goes only fades. */
export const moveLess = () => document.documentElement.classList.contains('less-motion');

/** Something small coming into place, such as an editor opening where its
 *  button was: it rises a few pixels as it fades in. With less motion, the
 *  fade alone. */
export function rise(node: Element, { y = 4, duration = 180 } = {}): TransitionConfig {
	if (moveLess()) return fade(node, { duration: 120 });
	return {
		duration,
		easing: settle,
		css: (t, u) => `opacity: ${t}; transform: translateY(${u * y}px)`
	};
}

/** Something small arriving as a mark, such as a count changing: it grows
 *  in from its middle. With less motion, a fade. */
export function pop(node: Element, { start = 0.6, duration = 200 } = {}): TransitionConfig {
	if (moveLess()) return fade(node, { duration: 120 });
	return {
		duration,
		easing: settle,
		css: (t) => `opacity: ${t}; scale: ${start + (1 - start) * t}`
	};
}

const SIDES = [
	'padding-top',
	'padding-bottom',
	'margin-top',
	'margin-bottom',
	'border-top-width',
	'border-bottom-width'
] as const;

/** The room an element takes, opening as it comes (`in:`) or closing as it
 *  goes (`out:`), so what is around it moves up or down rather than jumps.
 *  Its content fades with it. With less motion, a fade alone. */
export function room(node: Element, { duration = 300 } = {}): TransitionConfig {
	if (moveLess()) return fade(node, { duration: 150 });
	const style = getComputedStyle(node);
	const height = parseFloat(style.height);
	const sizes = SIDES.map((side) => parseFloat(style.getPropertyValue(side)));
	return {
		duration,
		easing: settle,
		css: (t) =>
			`overflow: hidden; opacity: ${t}; height: ${t * height}px; ` +
			SIDES.map((side, i) => `${side}: ${t * sizes[i]}px`).join('; ')
	};
}

/** Where an element sits inside a group, in its layout, whatever transform
 *  is running on it (a pin sliding to its new place reads where it lands). */
function placeIn(node: HTMLElement, group: HTMLElement) {
	let x = 0;
	let y = 0;
	let at: Element | null = node;
	while (at instanceof HTMLElement && at !== group) {
		x += at.offsetLeft;
		y += at.offsetTop;
		at = at.offsetParent;
	}
	if (at === group) return { x, y };
	const outer = group.getBoundingClientRect();
	const inner = node.getBoundingClientRect();
	return {
		x: inner.left - outer.left + group.scrollLeft,
		y: inner.top - outer.top + group.scrollTop
	};
}

/**
 * A mark that glides to whichever of a group is chosen, as the ribbon's bar
 * goes to the view shown or a tab's underline to the tab picked: one mark
 * moving says where the choice went, rather than one going out and another
 * lighting. Set on the mark, an element of the group (positioned) ahead of
 * what it marks; it follows the first of the group matching `chosen`, with
 * its place and size in `--mark-x`, `--mark-y`, `--mark-w` and `--mark-h`
 * (`.glide-mark` in layout.css draws it over them). While none is chosen it
 * fades out, and it comes back where the next one is rather than sliding
 * from where it last was.
 */
export function follow(chosen: string): Attachment<HTMLElement> {
	return (mark) => {
		const group = mark.parentElement;
		if (!group) return;
		let shown = false;
		let target: HTMLElement | null = null;
		/** Where the mark was last put, to skip a change that moves nothing. */
		let placed = '';
		// Also called as an element starts being watched, which is no resize:
		// the place it finds then is the one the mark is already gliding to.
		const sizes = new ResizeObserver(() => place(false));

		function place(glide: boolean) {
			const next = group!.querySelector<HTMLElement>(chosen);
			if (next !== target) {
				if (target) sizes.unobserve(target);
				if (next) sizes.observe(next);
				target = next;
			}
			if (!next) {
				mark.style.opacity = '0';
				shown = false;
				return;
			}
			const { x, y } = placeIn(next, group!);
			const [w, h] = [next.offsetWidth, next.offsetHeight];
			const at = `${x} ${y} ${w} ${h}`;
			if (shown && at === placed) return;
			placed = at;
			const jump = !glide || !shown;
			if (jump) mark.style.transition = 'none';
			mark.style.setProperty('--mark-x', `${x}px`);
			mark.style.setProperty('--mark-y', `${y}px`);
			mark.style.setProperty('--mark-w', `${w}px`);
			mark.style.setProperty('--mark-h', `${h}px`);
			if (jump) {
				// Takes the place at once; coming back, only its opacity eases.
				void mark.offsetWidth;
				mark.style.transition = '';
			}
			mark.style.opacity = '1';
			shown = true;
		}

		place(false);
		const changes = new MutationObserver(() => place(true));
		changes.observe(group, {
			subtree: true,
			childList: true,
			attributes: true,
			attributeFilter: ['data-state', 'aria-current', 'aria-checked', 'aria-selected']
		});
		sizes.observe(group);
		return () => {
			changes.disconnect();
			sizes.disconnect();
		};
	};
}
