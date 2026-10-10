import { commonmarkLanguage } from '@codemirror/lang-markdown';
import { Language, syntaxTree } from '@codemirror/language';
import { Facet, type Extension, type Range } from '@codemirror/state';
import {
	Decoration,
	ViewPlugin,
	type DecorationSet,
	type EditorView,
	type ViewUpdate
} from '@codemirror/view';
import type { MarkdownExtension, MarkdownParser } from '@lezer/markdown';
import { markdownExtensions, preview } from '#lib/markdown.js';
import { Attached, bullet, PluginWidget } from './widgets.js';

const hide = Decoration.replace({});
const markup = Decoration.mark({ class: 'md-markup' });

/** The space an editor's attachments are in, when it is not the open one. */
export const attachmentSpace = Facet.define<string, string | undefined>({
	combine: (spaces) => spaces[0]
});

/** The live preview's marks, hidden markup and widgets for the text as it is. */
function decorate(view: EditorView): DecorationSet {
	const { state } = view;
	const { doc } = state;
	const text = doc.toString();
	const { marks, hidden, lines, attachments, widgets } = preview(syntaxTree(state), text);

	// The lines the cursor or selection is on show their markup, to edit it.
	const shown = (pos: number) => {
		const line = doc.lineAt(pos);
		return (
			view.hasFocus &&
			state.selection.ranges.some((range) => range.from <= line.to && range.to >= line.from)
		);
	};

	const ranges: Range<Decoration>[] = [];
	for (const [from, line] of lines) {
		const attributes = line.style ? { style: line.style } : undefined;
		ranges.push(Decoration.line({ class: line.class, attributes }).range(from));
	}
	for (const mark of marks) {
		const attributes = mark.href ? { 'data-href': mark.href } : undefined;
		ranges.push(Decoration.mark({ class: mark.class, attributes }).range(mark.from, mark.to));
	}
	for (const range of hidden) {
		// A plugin may not hide a line break; a link split over two lines keeps its markup.
		if (range.from === range.to || text.slice(range.from, range.to).includes('\n')) continue;
		const deco = shown(range.from) ? markup : range.bullet ? bullet : hide;
		ranges.push(deco.range(range.from, range.to));
	}
	// A plugin's widget stands in for its node off the line being edited;
	// on that line the node shows as markup, to edit.
	for (const { from, to, text: nodeText, render, clicks } of widgets) {
		if (shown(from)) ranges.push(markup.range(from, to));
		else
			ranges.push(
				Decoration.replace({ widget: new PluginWidget(render, nodeText, clicks) }).range(from, to)
			);
	}
	// On the line being edited an attachment stays in view after its markup.
	const space = state.facet(attachmentSpace);
	for (const attached of attachments) {
		const { from, to } = attached;
		if (text.slice(from, to).includes('\n')) continue;
		const widget = new Attached(attached.path, attached.name, attached.image, space);
		if (shown(from)) {
			ranges.push(markup.range(from, to));
			ranges.push(Decoration.widget({ widget, side: 1 }).range(to));
		} else {
			ranges.push(Decoration.replace({ widget }).range(from, to));
		}
	}
	return Decoration.set(ranges, true);
}

const livePreview = () =>
	ViewPlugin.fromClass(
		class {
			decorations: DecorationSet;
			constructor(view: EditorView) {
				this.decorations = decorate(view);
			}
			update(update: ViewUpdate) {
				if (
					update.docChanged ||
					update.selectionSet ||
					update.focusChanged ||
					syntaxTree(update.startState) !== syntaxTree(update.state) ||
					update.startState.facet(attachmentSpace) !== update.state.facet(attachmentSpace)
				)
					this.decorations = decorate(update.view);
			}
		},
		{ decorations: (plugin) => plugin.decorations }
	);

/**
 * CommonMark's language data, so the editor's list keys recognise it, with
 * the syntax the read-only view parses: the core's and the plugins'.
 * Both are made again when a plugin's syntax comes or goes, and the live
 * preview with them, so it draws by the new rules at once.
 */
let support: { from: MarkdownExtension[]; extension: Extension } | undefined;
export function markdownSupport(): Extension {
	const from = markdownExtensions();
	if (support?.from !== from) {
		const parser = (commonmarkLanguage.parser as MarkdownParser).configure(from);
		const language = new Language(commonmarkLanguage.data, parser, [], 'markdown');
		support = { from, extension: [language, livePreview()] };
	}
	return support.extension;
}
