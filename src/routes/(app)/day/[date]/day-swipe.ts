import { onSwipe, phone } from '#lib/swipe.js';
import { inText } from '#lib/dom.js';
import type { Day } from './+page';

/** The day the finger brings in, beside the view, while it moves. */
export interface Sliding {
	day: Day;
	offset: number;
	top: number;
}

/** What the swipe asks of the day view. */
interface SwipeHost {
	/** Whether the day is alone in the view, which a swipe needs. */
	alone(): boolean;
	/** The day before the one shown, or after it. */
	beside(before: boolean): Day | undefined;
	/** The day sliding in, drawn beside the column. */
	sliding: Sliding | null;
	/** Open the day brought in, once it has landed. */
	open(day: Day): Promise<void>;
}

/**
 * When the day is alone in the view, a swipe on `main` slides in the day
 * before or after, as the arrows go to: `column` follows the finger, and
 * the day lands as the view opens on it. Where there is no day to go to,
 * the view holds back. Returns the way to stop.
 */
export function slideDays(main: HTMLElement, column: HTMLElement, host: SwipeHost) {
	/** How far apart the days sit: a column and the room on either side of it. */
	let gap = 0;
	const slide = (x: number, ms = 0) => {
		column.style.transition = ms ? `transform ${ms}ms cubic-bezier(0.2, 0.8, 0.2, 1)` : '';
		column.style.transform = x ? `translateX(${x}px)` : '';
	};
	const settle = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));
	const stop = onSwipe(main, {
		axis: 'x',
		// The left edge draws out the ribbon on a phone.
		accept: (touch, target) =>
			host.alone() && !inText(target) && !(phone.current && touch.clientX < 24),
		move: (x) => {
			const day = host.beside(x > 0);
			if (!day) {
				host.sliding = null;
				return slide(x * 0.15);
			}
			if (host.sliding?.day !== day) {
				const padding = parseFloat(getComputedStyle(main).paddingLeft);
				gap = column.offsetWidth + 2 * padding;
				// Hidden while it moves, or the day beyond the right edge would
				// scroll the view sideways.
				main.style.overflowX = 'hidden';
				host.sliding = { day, offset: x > 0 ? -gap : gap, top: main.scrollTop };
			}
			slide(x);
		},
		end: async (x, speed) => {
			const sliding = host.sliding;
			const day = sliding?.day;
			const far = Math.abs(x) > gap * 0.3 || (Math.abs(x) > 30 && Math.abs(speed) > 0.4);
			if (!day || !far || Math.sign(x) !== Math.sign(-sliding!.offset)) {
				slide(0, 200);
				await settle(200);
				if (column.style.transform === '') host.sliding = null;
				main.style.overflowX = '';
				return;
			}
			slide(-sliding!.offset, 220);
			await settle(220);
			// Both to the top, which the view opens on, in one frame: the day
			// brought in stays where it shows.
			main.scrollTop = 0;
			host.sliding = { ...host.sliding!, top: 0 };
			try {
				await host.open(day);
			} finally {
				slide(0);
				host.sliding = null;
				main.style.overflowX = '';
			}
		}
	});
	return () => {
		stop();
		slide(0);
		main.style.overflowX = '';
	};
}
