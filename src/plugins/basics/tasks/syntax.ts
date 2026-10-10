import { TaskList } from '@lezer/markdown';
import type { MarkdownSyntax, WidgetContext } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import { BOX } from './tasks';

/** The task last ticked on a card, by its line's text, and when: the card
 *  draws the note anew with a new box, which then draws its tick in. It
 *  is drawn again as the note saves, and that box goes on where it was. */
let justTicked: { line: string; at: number } | null = null;
const TICKING_MS = 400;

/** The text of the line a box is on, which names its task. */
const lineOf = (box: HTMLElement) => box.closest('.md-line')?.textContent?.trim() ?? '';

/**
 * A task's box in place of its `[ ]` (SPEC 3.4). A click ticks or clears
 * it: in the editor it changes the text being written and leaves the
 * cursor where it was, on a card it saves the note at once.
 */
function box(node: WidgetContext): HTMLElement {
	const done = node.text[1] !== ' ';
	const toggle = () => node.update(done ? '[ ]' : '[x]');
	const className = done ? 'md-task md-ticked' : 'md-task';

	if (node.where === 'editor') {
		const span = document.createElement('span');
		span.className = className;
		span.addEventListener('mousedown', (event) => {
			event.preventDefault();
			toggle();
		});
		return span;
	}

	// A preview, and a card that does not save, draw it without a click.
	const button = document.createElement('button');
	button.type = 'button';
	button.className = className;
	button.setAttribute('role', 'checkbox');
	button.setAttribute('aria-checked', String(done));
	button.setAttribute('aria-label', m.note_task_done());
	button.disabled = !node.editable;
	button.addEventListener('click', () => {
		if (!done) justTicked = { line: lineOf(button), at: performance.now() };
		toggle();
	});
	// Named after its task once it is on the page, so a screen reader says
	// which task it ticks; "Done" stays for a box with no text after it.
	queueMicrotask(() => {
		const task = lineOf(button);
		if (task) button.setAttribute('aria-label', task);
		const since = justTicked ? performance.now() - justTicked.at : Infinity;
		if (done && justTicked?.line === task && since < TICKING_MS) {
			button.classList.add('md-tick-in');
			button.style.setProperty('--ticked-ago', `${-since}ms`);
		}
	});
	return button;
}

/** GFM's task lists, `- [ ]` and `- [x]`, with their box drawn in the bullet's place. */
export const taskSyntax: MarkdownSyntax = {
	extension: TaskList,
	itemMarks: [BOX],
	render: {
		Task: { replacesBullet: true },
		TaskMarker: { widget: box, spaces: true, handlesClicks: true }
	}
};
