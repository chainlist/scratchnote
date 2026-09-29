import type { EditorView } from '@codemirror/view';
import { isList, isMarked, toggleList, toggleMark } from '$lib/format';

/**
 * The editor a plugin's command or toolbar button acts on: a note, a page,
 * or the capture window being written in. The helpers do what the app's
 * own toolbar does, so a plugin's format behaves as bold or a list does;
 * `cm` is the CodeMirror view underneath, for anything else.
 */
export class Editor {
	readonly cm: EditorView;

	constructor(cm: EditorView) {
		this.cm = cm;
	}

	/** The whole text. */
	getValue(): string {
		return this.cm.state.doc.toString();
	}

	setValue(text: string) {
		this.cm.dispatch({ changes: { from: 0, to: this.cm.state.doc.length, insert: text } });
	}

	getSelection(): string {
		const { from, to } = this.cm.state.selection.main;
		return this.cm.state.sliceDoc(from, to);
	}

	/** Put `text` in place of the selection, or at the cursor. */
	replaceSelection(text: string) {
		this.cm.dispatch(this.cm.state.replaceSelection(text));
	}

	/**
	 * An inline format as bold is one: `mark` around the selection, the word
	 * at the cursor, or what is typed next, and off again where the text
	 * has it. `node` is what the format parses to, from the plugin's syntax.
	 */
	toggleMark(node: string, mark: string) {
		toggleMark(node, mark)(this.cm);
	}

	/** Whether the selection has the format of `node`, written with `mark`. */
	hasMark(node: string, mark: string): boolean {
		return isMarked(this.cm.state, node, mark);
	}

	/**
	 * Makes the selected lines a list whose items start with `mark`, or
	 * plain lines when they all are one. A list whose items carry more than
	 * a bullet, as a checklist's box, passes that item mark from its syntax.
	 */
	toggleList(mark: string, itemMark?: RegExp) {
		toggleList(itemMark ?? 'bullet', mark)(this.cm);
	}

	/** Whether every selected line is an item of that list. */
	isList(itemMark?: RegExp): boolean {
		return isList(this.cm.state, itemMark ?? 'bullet');
	}

	focus() {
		this.cm.focus();
	}
}
