import { quintOut } from 'svelte/easing';
import { fade, type TransitionConfig } from 'svelte/transition';

/** The easing of what arrives, as `page-in` has it in layout.css:
 *  `cubic-bezier(0.22, 1, 0.36, 1)`, quick and then settling. */
export const settle = quintOut;

/** Whether the user asked for less motion: then nothing moves across the
 *  screen, and what comes or goes only fades. */
export const moveLess = () => matchMedia('(prefers-reduced-motion: reduce)').matches;

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
