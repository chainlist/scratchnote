import { TaskList } from '@lezer/markdown';
import type { MarkdownSyntax, WidgetContext } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import { BOX } from './tasks';

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
	button.addEventListener('click', toggle);
	// Named after its task once it is on the page, so a screen reader says
	// which task it ticks; "Done" stays for a box with no text after it.
	queueMicrotask(() => {
		const task = button.closest('.md-line')?.textContent?.trim();
		if (task) button.setAttribute('aria-label', task);
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
