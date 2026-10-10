import { element } from '../dom';

/** Turning a page: a leaf that swings over the book from one spread to the next. */

/** How long a page takes to turn, in milliseconds. */
const TURN = 450;
/** Slow off the page, quick through the air, gentle on landing. */
const TURN_EASING = 'cubic-bezier(0.45, 0.05, 0.25, 1)';

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** Half of a spread, copied, for one side of the page being turned, and the
 *  shade that darkens it as it turns. */
function face(spread: HTMLElement, half: 'left' | 'right') {
	const el = element('div', `journal-face journal-face-${half}`);
	const copy = spread.cloneNode(true) as HTMLElement;
	copy.classList.remove('journal-only-left', 'journal-only-right');
	const shade = element('div', 'journal-shade');
	el.append(copy, shade);
	return { el, shade };
}

/**
 * Turn a leaf over `pages` from the spread `current` to `target`, drawn
 * under it, forward or back. Resolves to false when `closed` says the view
 * closed meanwhile, the turn left where it was.
 */
export async function animateTurn(
	pages: HTMLElement,
	current: HTMLElement,
	target: HTMLElement,
	forward: boolean,
	closed: () => boolean
): Promise<boolean> {
	// The leaf: this spread's page on its front, the next one's on its back.
	const leaf = element('div', `journal-leaf journal-leaf-${forward ? 'next' : 'previous'}`);
	leaf.inert = true;
	const front = face(current, forward ? 'right' : 'left');
	const back = face(target, forward ? 'left' : 'right');
	back.el.classList.add('journal-back');
	leaf.append(front.el, back.el);
	await Promise.all([...leaf.querySelectorAll('img')].map((img) => img.decode().catch(() => {})));
	if (closed()) return false;
	// Still while it turns: the other page of this spread, and the page it uncovers.
	current.classList.add(forward ? 'journal-only-left' : 'journal-only-right');
	target.classList.add(forward ? 'journal-only-right' : 'journal-only-left');
	const uncovered = element('div', `journal-cast journal-cast-${forward ? 'right' : 'left'}`);
	const landing = element('div', `journal-cast journal-cast-${forward ? 'left' : 'right'}`);
	pages.classList.add('journal-turning');
	pages.append(uncovered, landing, leaf);
	const options: KeyframeAnimationOptions = {
		duration: TURN,
		easing: TURN_EASING,
		fill: 'forwards'
	};
	const turning = leaf.animate(
		[{ transform: 'rotateY(0deg)' }, { transform: `rotateY(${forward ? -180 : 180}deg)` }],
		options
	);
	front.shade.animate([{ opacity: 0 }, { opacity: 0.4, offset: 0.5 }, { opacity: 0.4 }], options);
	back.shade.animate([{ opacity: 0.4 }, { opacity: 0.4, offset: 0.5 }, { opacity: 0 }], options);
	uncovered.animate([{ opacity: 0.8 }, { opacity: 0, offset: 0.6 }, { opacity: 0 }], options);
	landing.animate(
		[{ opacity: 0 }, { opacity: 0, offset: 0.45 }, { opacity: 0.7, offset: 0.9 }, { opacity: 0 }],
		options
	);
	// A window out of sight may not run the animation; the page turns anyway.
	await Promise.race([turning.finished, wait(TURN + 400)]);
	if (closed()) return false;
	leaf.remove();
	uncovered.remove();
	landing.remove();
	pages.classList.remove('journal-turning');
	target.classList.remove('journal-only-left', 'journal-only-right');
	return true;
}
