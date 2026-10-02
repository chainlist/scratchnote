import type { MarkdownConfig } from '@lezer/markdown';
import { Component } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import type { BasicsPlugin } from '../plugin.svelte';

/** Lucide's highlighter. */
export const HIGHLIGHTER =
	'<path d="m9 11-6 6v3h9l3-3"/><path d="m22 12-4.6 4.6a2 2 0 0 1-2.8 0l-5.2-5.2a2 2 0 0 1 0-2.8L14 4"/>';

/** What highlighted text can be painted with, and their names. */
export const COLORS = {
	yellow: m.highlights_color_yellow,
	green: m.highlights_color_green,
	blue: m.highlights_color_blue,
	pink: m.highlights_color_pink,
	accent: m.settings_accent
};

export type HighlightColor = keyof typeof COLORS;

/** `==` opens and closes a highlight, as `~~` does a strikethrough. */
const DELIMITER = { resolve: 'Highlight', mark: 'HighlightMark' };
const EQUALS = 61;

const highlight: MarkdownConfig = {
	defineNodes: ['Highlight', 'HighlightMark'],
	parseInline: [
		{
			name: 'Highlight',
			parse(cx, next, pos) {
				if (next !== EQUALS || cx.char(pos + 1) !== EQUALS || cx.char(pos + 2) === EQUALS)
					return -1;
				// Before a space it can only close, after one only open.
				const before = cx.slice(pos - 1, pos);
				const after = cx.slice(pos + 2, pos + 3);
				const open = after !== '' && !/\s/.test(after);
				const close = before !== '' && !/\s/.test(before);
				return cx.addDelimiter(DELIMITER, pos, pos + 2, open, close);
			},
			after: 'Emphasis'
		}
	]
};

/**
 * Highlights, a part of Basics (SPEC 3.8): `==text==` on a band of color,
 * from the toolbar, the command center or Ctrl+Shift+H. Switched off, `==`
 * shows as typed.
 */
export class Highlights extends Component {
	#plugin: BasicsPlugin;

	constructor(plugin: BasicsPlugin) {
		super();
		this.#plugin = plugin;
	}

	onload() {
		this.registerMarkdownSyntax({
			extension: highlight,
			render: {
				Highlight: { class: 'hl-mark' },
				HighlightMark: { hide: true }
			}
		});
		this.addCommand({
			id: 'highlight',
			name: m.highlights_command,
			icon: HIGHLIGHTER,
			hotkey: 'Mod-Shift-h',
			editorCallback: (editor) => editor.toggleMark('Highlight', '==')
		});
		this.addToolbarButton({
			id: 'highlight',
			icon: HIGHLIGHTER,
			title: m.format_highlight,
			group: 'text',
			run: (editor) => editor.toggleMark('Highlight', '=='),
			active: (editor) => editor.hasMark('Highlight', '==')
		});
		this.register(() => delete document.documentElement.dataset.highlights);
		this.update();
	}

	/** The color is a variable the stylesheet reads, set on the page. */
	update() {
		document.documentElement.dataset.highlights = this.#plugin.settings.highlights.color;
	}
}
