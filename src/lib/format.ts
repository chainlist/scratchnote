import { syntaxTree } from '@codemirror/language';
import type { ChangeSpec, EditorState, StateCommand } from '@codemirror/state';
import type { SyntaxNode } from '@lezer/common';
import { itemMarks } from '#lib/markdown.js';

/**
 * What the editor's toolbar and shortcuts do (SPEC 3.4): each writes the
 * markdown for a format, or takes it away again, so the text stays the
 * markdown it always was.
 */

/** Indentation and quote marks at the start of a line, which a list keeps. */
const LEAD = /^(?:[ \t]*>)*[ \t]*/;
/** A list mark or a heading's `#`s after the lead. */
const BLOCK = /^(?:[-*+][ \t]+|\d+[.)][ \t]+|#{1,6}[ \t]+)/;

/**
 * What a list line is: a plain bullet, a bullet carrying one of the item
 * marks the plugins' syntax adds (a task's box), or another list or heading.
 */
type Kind = 'bullet' | RegExp | 'other';

/**
 * Where a line's lead ends, where its text starts, and what kind of line it
 * is. An item mark only counts while a plugin's syntax adds it, so with the
 * Tasks plugin off `- [ ] milk` is a bullet whose text starts `[ ]`.
 */
function block(text: string) {
	const lead = LEAD.exec(text)![0].length;
	const match = BLOCK.exec(text.slice(lead));
	let start = lead + (match?.[0].length ?? 0);
	let kind: Kind | null = !match ? null : /^[-*+]/.test(match[0]) ? 'bullet' : 'other';
	if (kind === 'bullet') {
		for (const mark of itemMarks()) {
			const sticky = new RegExp(mark.source, 'y');
			sticky.lastIndex = start;
			const item = sticky.exec(text);
			if (item) {
				kind = mark;
				start += item[0].length;
				break;
			}
		}
	}
	return { lead, start, kind };
}

/**
 * The lines with text on them that the selection covers, or the cursor's
 * line when none has any. A selection ending at a line's start, as a triple
 * click leaves it, stops at the line before.
 */
function selectedLines(state: EditorState) {
	const { from, to } = state.selection.main;
	const first = state.doc.lineAt(from).number;
	let last = state.doc.lineAt(to).number;
	if (to > from && state.doc.line(last).from === to) last--;
	const lines = [];
	for (let n = first; n <= last; n++) lines.push(state.doc.line(n));
	const written = lines.filter((line) => line.text.trim() !== '');
	return written.length ? written : lines;
}

export const isList = (state: EditorState, kind: 'bullet' | RegExp) =>
	selectedLines(state).every((line) => block(line.text).kind === kind);

/**
 * Makes the selected lines a list of this kind, or plain lines when they all
 * are one already. Lines that are one keep their mark, and a task its tick.
 * A list whose lines carry an item mark, a checklist, is named by that mark.
 */
export function toggleList(kind: 'bullet' | RegExp, mark: string): StateCommand {
	return ({ state, dispatch }) => {
		const all = isList(state, kind);
		const changes: ChangeSpec[] = [];
		for (const line of selectedLines(state)) {
			const { lead, start, kind: was } = block(line.text);
			if (all || was !== kind)
				changes.push({ from: line.from + lead, to: line.from + start, insert: all ? '' : mark });
		}
		const set = state.changes(changes);
		dispatch(
			state.update({
				changes: set,
				selection: state.selection.map(set, 1),
				scrollIntoView: true,
				userEvent: 'input.format'
			})
		);
		return true;
	};
}

/** The innermost node named `name` holding the selection, or the cursor strictly inside it. */
function enclosing(state: EditorState, name: string): SyntaxNode | null {
	const { from, to, empty } = state.selection.main;
	for (let n: SyntaxNode | null = syntaxTree(state).resolveInner(from, 1); n; n = n.parent) {
		if (n.name === name && n.from <= from && n.to >= to && (!empty || n.from < from)) return n;
	}
	return null;
}

/**
 * The cursor sits between an empty pair of the mark, `**|**`, and not in a
 * longer run of its character.
 */
function inEmptyPair(state: EditorState, mark: string) {
	const { head, empty } = state.selection.main;
	const at = (from: number, to: number) => state.sliceDoc(from, to);
	const n = mark.length;
	return (
		empty &&
		at(head - n, head) === mark &&
		at(head, head + n) === mark &&
		at(head - n - 1, head - n) !== mark[0] &&
		at(head + n, head + n + 1) !== mark[0]
	);
}

/** Whether the selection has the format that parses to `node`, written with `mark`. */
export const isMarked = (state: EditorState, node: string, mark: string) =>
	enclosing(state, node) !== null || inEmptyPair(state, mark);

/**
 * Bold or italic, as a word processor does them: on the selection, or on
 * the word the cursor is in, or on what is typed next. Again on text that
 * has it takes it away; at the end of it, the cursor steps out so typing
 * goes on plain. `node` is what the format parses to, opened and closed by
 * its first and last child, so a plugin's inline syntax (`==highlight==`)
 * toggles the same way.
 */
export function toggleMark(node: string, mark: string): StateCommand {
	return ({ state, dispatch }) => {
		const range = state.selection.main;
		const update = (changes: ChangeSpec[], cursor?: number) => {
			const set = state.changes(changes);
			dispatch(
				state.update({
					changes: set,
					selection: cursor === undefined ? state.selection.map(set, 1) : { anchor: cursor },
					scrollIntoView: true,
					userEvent: 'input.format'
				})
			);
			return true;
		};

		const marked = enclosing(state, node);
		const open = marked?.firstChild;
		const close = marked?.lastChild;
		if (open && close && open.from !== close.from) {
			if (range.empty && range.head === close.from) {
				dispatch(state.update({ selection: { anchor: close.to }, userEvent: 'select' }));
				return true;
			}
			return update([
				{ from: open.from, to: open.to },
				{ from: close.from, to: close.to }
			]);
		}

		if (range.empty) {
			const { head } = range;
			if (inEmptyPair(state, mark))
				return update([{ from: head - mark.length, to: head + mark.length }], head - mark.length);
			const word = state.wordAt(head);
			if (word && word.from < head && head < word.to)
				return update([
					{ from: word.from, insert: mark },
					{ from: word.to, insert: mark }
				]);
			return update([{ from: head, insert: mark + mark }], head + mark.length);
		}

		// Line by line, past a list mark or heading, and without the spaces
		// at either end, which would stop the markdown from reading as bold.
		const changes: ChangeSpec[] = [];
		for (let pos = range.from; pos <= range.to;) {
			const line = state.doc.lineAt(pos);
			let from = Math.max(range.from, line.from + block(line.text).start);
			let to = Math.min(range.to, line.to);
			while (from < to && /\s/.test(state.sliceDoc(from, from + 1))) from++;
			while (to > from && /\s/.test(state.sliceDoc(to - 1, to))) to--;
			if (from < to) changes.push({ from, insert: mark }, { from: to, insert: mark });
			pos = line.to + 1;
		}
		return changes.length > 0 && update(changes);
	};
}

export const bold = toggleMark('StrongEmphasis', '**');
export const italic = toggleMark('Emphasis', '*');
export const bullets = toggleList('bullet', '- ');

/**
 * Makes the selection, or the word at the cursor, a link, with `url`
 * selected for the address to be pasted over it. In a link already, its
 * address is selected.
 */
export const link: StateCommand = ({ state, dispatch }) => {
	const url = enclosing(state, 'Link')?.getChild('URL');
	if (url) {
		dispatch(state.update({ selection: { anchor: url.from, head: url.to }, scrollIntoView: true }));
		return true;
	}
	let { from, to } = state.selection.main;
	const word = from === to ? state.wordAt(from) : null;
	if (word) ({ from, to } = word);
	const text = state.sliceDoc(from, to);
	const start = from + text.length + 3;
	dispatch(
		state.update({
			changes: { from, to, insert: `[${text}](url)` },
			// Without text, the cursor waits for it between the brackets.
			selection: text ? { anchor: start, head: start + 3 } : { anchor: from + 1 },
			scrollIntoView: true,
			userEvent: 'input.format'
		})
	);
	return true;
};

/** Which formats the selection has, for the toolbar to show pressed. */
export const formats = (state: EditorState) => ({
	bold: isMarked(state, 'StrongEmphasis', '**'),
	italic: isMarked(state, 'Emphasis', '*'),
	bullets: isList(state, 'bullet')
});
