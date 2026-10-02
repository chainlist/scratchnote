import type { MarkdownRenderer } from '#lib/plugins/api.js';

/** A task: its box by the offset of its `[`, and its first line from the list mark on. */
export type Task = { at: number; done: boolean; from: number; to: number };

/** A task's box after a list bullet, for the list buttons to keep it with the bullet. */
export const BOX = /\[[ xX]\](?:[ \t]+|$)/;

/** The tasks in a text, read as the cards read them. */
export function tasks(markdown: MarkdownRenderer, text: string): Task[] {
	const found: Task[] = [];
	markdown.parse(text).iterate({
		enter(node) {
			if (node.name !== 'TaskMarker') return;
			// TaskMarker sits in the Task, which sits in the list item.
			const mark = node.node.parent?.parent?.getChild('ListMark');
			const end = text.indexOf('\n', node.from);
			found.push({
				at: node.from,
				done: text[node.from + 1] !== ' ',
				from: mark?.from ?? node.from,
				to: end < 0 ? text.length : end
			});
		}
	});
	return found;
}

/** The text with the task box whose `[` is at `at` ticked, or cleared if it was. */
export const toggleTask = (text: string, at: number) =>
	text.slice(0, at + 1) + (text[at + 1] === ' ' ? 'x' : ' ') + text.slice(at + 2);
