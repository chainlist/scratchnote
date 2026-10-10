import {
	Autolink,
	Strikethrough,
	parser as commonmark,
	type MarkdownExtension,
	type MarkdownParser
} from '@lezer/markdown';
import type { SyntaxNode, Tree } from '@lezer/common';
import { mentionRender, mentionSyntax } from '#lib/mentions.js';
import type { NodeRender, WidgetContext } from '#lib/plugins/types.js';

import { registry, type SyntaxEntry } from '#lib/plugins/registry.svelte.js';

/**
 * The markdown notes are shown with: CommonMark, bare links,
 * ~~strikethrough~~ and `@name` mentions (SPEC 3.10), and what the plugins'
 * syntax adds, such as the Tasks core plugin's `- [ ]` boxes (SPEC 3.9).
 * The editor and the read-only view both go through `preview`, so a note
 * looks the same written and read.
 */
const CORE: MarkdownExtension[] = [Strikethrough, Autolink, mentionSyntax];

/** How the app draws the nodes of its own syntax. */
const CORE_RULES: [string, NodeRender][] = [['Mention', mentionRender]];

interface Syntax {
	from: SyntaxEntry[];
	extensions: MarkdownExtension[];
	parser: MarkdownParser;
	/** How the plugins draw their nodes, by name. */
	rules: Map<string, NodeRender>;
	/** Nodes drawn in a list item's bullet's place, as a task's box is. */
	bulletless: string[];
	/** What a list item may carry after its bullet, for the list commands. */
	itemMarks: RegExp[];
}

let built: Syntax | undefined;

/**
 * The syntax as the plugins have it now, rebuilt when one comes or goes.
 * Read in an effect or a derived, it is followed: a card redraws when a
 * plugin's syntax loads.
 */
function current(): Syntax {
	const from = registry.syntax;
	if (built?.from === from) return built;
	const extensions = [...CORE];
	const rules = new Map<string, NodeRender>(CORE_RULES);
	const bulletless: string[] = [];
	const itemMarks: RegExp[] = [];
	for (const { syntax } of from) {
		if (syntax.extension) extensions.push(syntax.extension);
		for (const [name, rule] of Object.entries(syntax.render ?? {})) {
			rules.set(name, rule);
			if (rule.replacesBullet) bulletless.push(name);
		}
		itemMarks.push(...(syntax.itemMarks ?? []));
	}
	built = {
		from,
		extensions,
		parser: commonmark.configure(extensions),
		rules,
		bulletless,
		itemMarks
	};
	return built;
}

/** The syntax extensions in use, for the editor's language. */
export const markdownExtensions = () => current().extensions;

/** The tree the cards and the editor read `text` with. */
export const parseMarkdown = (text: string) => current().parser.parse(text);

/** What a list item may carry after its bullet, such as a task's box. */
export const itemMarks = () => current().itemMarks;

/** What a plugin's widget draws itself from. */
export type WidgetRender = (node: WidgetContext) => HTMLElement;

export type Preview = {
	/** Styled stretches; a link carries where it opens. */
	marks: { from: number; to: number; class: string; href?: string }[];
	/**
	 * Markup hidden while the cursor is off its line: `**`, `](url)`, `# `.
	 * A bullet stands in for a list mark.
	 */
	hidden: { from: number; to: number; bullet?: boolean }[];
	/** Classes and styles for whole lines, by the offset each line starts at. */
	lines: Map<number, { class: string; style?: string }>;
	/**
	 * Links to attached files, each drawn in place of its markup: the image
	 * for `![name](path)` to an image, a card for any other.
	 */
	attachments: { from: number; to: number; path: string; name: string; image: boolean }[];
	/**
	 * Nodes a plugin draws itself, such as a task's box, in place of the
	 * stretch `from` to `to`: the node's `text`, and the spaces after it
	 * when its rule takes them.
	 */
	widgets: { from: number; to: number; text: string; render: WidgetRender; clicks: boolean }[];
};

/** Width of a list bullet and the gap after it; nested lists step in by as much. */
const BULLET_EM = 1.25;

/** Where a link opens, or nothing when it is not a web or mail link. */
function href(url: string): string | undefined {
	if (/^www\./i.test(url)) return `https://${url}`;
	if (/^(https?|mailto):/i.test(url)) return url;
	if (!url.includes(':') && url.includes('@')) return `mailto:${url}`;
	return undefined;
}

/** The files an attachment link shows as a picture; the backend serves them as images. */
export const isImage = (path: string) => /\.(png|jpe?g|gif|webp|avif|bmp|svg|ico)$/i.test(path);

/**
 * The way from a note's file up to its space's folder. Day files and page
 * files both sit two folders down, so one link reaches an attachment from
 * either (SPEC 4.8).
 */
const TO_SPACE = '../../';

/**
 * The attached file a link points at, from the space's folder
 * (`attachments/2026/…`), or nothing when it points elsewhere.
 */
export function attachmentPath(url: string): string | undefined {
	let path = url.startsWith('<') && url.endsWith('>') ? url.slice(1, -1) : url;
	try {
		path = decodeURI(path);
	} catch {
		// A stray `%` stays as typed.
	}
	return path.startsWith(`${TO_SPACE}attachments/`) ? path.slice(TO_SPACE.length) : undefined;
}

/** The markdown linking an attachment: an image shows in the text, any other file as a card. */
export function attachmentLink(attachment: { name: string; path: string }): string {
	const text = attachment.name.replace(/[\\`*_[\]]/g, '\\$&');
	return `${isImage(attachment.path) ? '!' : ''}[${text}](<${TO_SPACE}${attachment.path}>)`;
}

/** `text` with the links `attachmentLink` wrote to the file at `from` pointed at `to` instead. */
export function relink(text: string, from: string, to: string): string {
	return text.replaceAll(`(<${TO_SPACE}${from}>)`, `(<${TO_SPACE}${to}>)`);
}

/** The last part of a path, for an attachment linked without a name. */
export const fileName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

/** A file's type as its card shows it: a short extension in capitals, or nothing. */
export const fileType = (path: string) =>
	/\.([a-z0-9]{1,4})$/i.exec(fileName(path))?.[1].toUpperCase() ?? '';

/** A file's name on its card, without the extension its type already shows. */
export function cardName(name: string, path: string): string {
	const type = fileType(path);
	const stem = name.slice(0, -type.length - 1);
	return type && stem && name.toUpperCase().endsWith(`.${type}`) ? stem : name;
}

export function preview(tree: Tree, text: string): Preview {
	const { rules, bulletless } = current();
	const marks: Preview['marks'] = [];
	const hidden: Preview['hidden'] = [];
	const lines: Preview['lines'] = new Map();
	const attachments: Preview['attachments'] = [];
	const widgets: Preview['widgets'] = [];
	/** Link text as it reads, its backslash escapes undone. */
	const unescape = (from: number, to: number) => text.slice(from, to).replace(/\\(.)/g, '$1');

	const lineStart = (pos: number) => text.lastIndexOf('\n', pos - 1) + 1;
	const addLine = (start: number, cls: string, style?: string) => {
		const line = lines.get(start);
		lines.set(start, {
			class: line ? `${line.class} ${cls}` : cls,
			style: style ?? line?.style
		});
	};
	const addLines = (from: number, to: number, cls: string) => {
		for (let start = lineStart(from); ;) {
			addLine(start, cls);
			const next = text.indexOf('\n', start) + 1;
			if (next === 0 || next > to) break;
			start = next;
		}
	};
	/** Markup that leads a line (`# `, `> `) takes the spaces after it along. */
	const hideLeading = (from: number, to: number) => {
		while (text[to] === ' ') to++;
		hidden.push({ from, to });
	};

	tree.iterate({
		enter(node) {
			const { from, to } = node;
			const parent = node.node.parent?.name;
			switch (node.name) {
				case 'StrongEmphasis':
					marks.push({ from, to, class: 'md-strong' });
					break;
				case 'Emphasis':
					marks.push({ from, to, class: 'md-em' });
					break;
				case 'Strikethrough':
					marks.push({ from, to, class: 'md-strike' });
					break;
				case 'InlineCode':
					marks.push({ from, to, class: 'md-code' });
					break;
				case 'EmphasisMark':
				case 'StrikethroughMark':
				case 'CodeMark':
				case 'CodeInfo':
					hidden.push({ from, to });
					break;
				case 'Escape':
					hidden.push({ from, to: from + 1 });
					break;
				case 'HeaderMark':
					// Only the opening `#`s; a setext underline stays as typed.
					if (parent?.startsWith('ATX') && from === node.node.parent!.from) hideLeading(from, to);
					break;
				case 'ATXHeading1':
				case 'ATXHeading2':
				case 'ATXHeading3':
				case 'ATXHeading4':
				case 'ATXHeading5':
				case 'ATXHeading6':
					addLine(lineStart(from), `md-h${node.name.slice(-1)}`);
					break;
				case 'QuoteMark':
					hideLeading(from, to);
					break;
				case 'Blockquote':
					addLines(from, to, 'md-quote');
					break;
				case 'FencedCode':
					addLines(from, to, 'md-codeblock');
					break;
				case 'ListMark': {
					if (node.node.parent?.parent?.name !== 'BulletList') break;
					// The indent goes too: the line steps in by depth instead, so
					// wrapped text lines up under the first word.
					let depth = 0;
					for (let n: SyntaxNode | null = node.node.parent; n; n = n.parent)
						if (n.name === 'BulletList' || n.name === 'OrderedList') depth++;
					let start = from;
					while (start > 0 && (text[start - 1] === ' ' || text[start - 1] === '\t')) start--;
					let end = to;
					while (text[end] === ' ') end++;
					// What a plugin draws in the bullet's place, a task's box, takes it.
					const item = node.node.parent;
					const bullet = !bulletless.some((name) => item.getChild(name) !== null);
					hidden.push({ from: start, to: end, bullet });
					addLine(
						lineStart(from),
						'md-li',
						`padding-left: calc(var(--md-indent, 0em) + ${depth * BULLET_EM}em); text-indent: -${BULLET_EM}em`
					);
					break;
				}
				case 'Link': {
					const url = node.node.getChild('URL');
					const close = node.node.getChildren('LinkMark').find((m) => text[m.from] === ']');
					// A reference link, `[text][ref]`, is left as typed.
					if (!url || !close) break;
					const target = text.slice(url.from, url.to);
					const attachment = attachmentPath(target);
					if (attachment) {
						const name = unescape(from + 1, close.from);
						attachments.push({ from, to, path: attachment, name, image: false });
						return false;
					}
					marks.push({ from: from + 1, to: close.from, class: 'md-link', href: href(target) });
					hidden.push({ from, to: from + 1 }, { from: close.from, to });
					break;
				}
				case 'Autolink':
					// `<https://…>`: the brackets go, the URL inside shows as a link.
					hidden.push({ from, to: from + 1 }, { from: to - 1, to });
					break;
				case 'URL':
					if (parent === 'Link' || parent === 'Image' || parent === 'LinkReference') break;
					marks.push({ from, to, class: 'md-link', href: href(text.slice(from, to)) });
					break;
				case 'Image': {
					const url = node.node.getChild('URL');
					const close = node.node.getChildren('LinkMark').find((m) => text[m.from] === ']');
					const attachment =
						url && close ? attachmentPath(text.slice(url.from, url.to)) : undefined;
					// Any other image, one on the web say, is left as typed: nothing is fetched.
					if (!attachment || !close) return false;
					const name = unescape(from + 2, close.from);
					attachments.push({ from, to, path: attachment, name, image: isImage(attachment) });
					return false;
				}
			}

			// What a plugin says about the node, on top of what the app does.
			const rule = rules.get(node.name);
			if (!rule) return;
			let end = to;
			if (rule.spaces) while (text[end] === ' ') end++;
			if (rule.class) marks.push({ from, to, class: rule.class });
			if (rule.line) addLines(from, to, rule.line);
			// A widget stands in for one line's worth; a node over several shows as typed.
			const nodeText = text.slice(from, to);
			if (rule.widget && !nodeText.includes('\n')) {
				const clicks = rule.handlesClicks ?? false;
				widgets.push({ from, to: end, text: nodeText, render: rule.widget, clicks });
				return false;
			}
			if (rule.hide) hidden.push({ from, to: end });
		}
	});

	return { marks, hidden, lines, attachments, widgets };
}

export type Part =
	| { text: string; class: string; href?: string }
	| { bullet: true }
	/** A plugin's widget for the node `text` at `from`. */
	| { widget: WidgetRender; text: string; from: number }
	| { attachment: string; name: string; image: boolean };
export type Line = { class: string; style?: string; parts: Part[] };

/** The note as lines of styled text with its markup hidden, for reading. */
export function renderLines(text: string): Line[] {
	const { marks, hidden, lines, attachments, widgets } = preview(parseMarkdown(text), text);

	const classes = new Array<string>(text.length).fill('');
	const hrefs = new Array<string | undefined>(text.length);
	for (const mark of marks) {
		for (let i = mark.from; i < mark.to; i++) {
			classes[i] += ` ${mark.class}`;
			if (mark.href) hrefs[i] = mark.href;
		}
	}
	// 1 hides a character, 2 draws a bullet in its place, 3 an attachment,
	// 4 a plugin's widget.
	const skip = new Uint8Array(text.length);
	for (const range of hidden) {
		skip.fill(1, range.from, range.to);
		if (range.bullet) skip[range.from] = 2;
	}
	const attachmentAt = new Map(attachments.map((attached) => [attached.from, attached]));
	for (const attached of attachments) {
		skip.fill(1, attached.from, attached.to);
		skip[attached.from] = 3;
	}
	const widgetAt = new Map(widgets.map((widget) => [widget.from, widget]));
	for (const widget of widgets) {
		skip.fill(1, widget.from, widget.to);
		skip[widget.from] = 4;
	}

	const out: Line[] = [];
	let start = 0;
	for (const raw of text.split('\n')) {
		const end = start + raw.length;
		const parts: Part[] = [];
		for (let i = start; i < end;) {
			if (skip[i]) {
				if (skip[i] === 2) parts.push({ bullet: true });
				if (skip[i] === 4) {
					const { render, text: nodeText } = widgetAt.get(i)!;
					parts.push({ widget: render, text: nodeText, from: i });
				}
				if (skip[i] === 3) {
					const { path, name, image } = attachmentAt.get(i)!;
					parts.push({ attachment: path, name, image });
				}
				i++;
				continue;
			}
			let j = i + 1;
			while (j < end && !skip[j] && classes[j] === classes[i] && hrefs[j] === hrefs[i]) j++;
			parts.push({ text: text.slice(i, j), class: classes[i].trim(), href: hrefs[i] });
			i = j;
		}
		const line = lines.get(start);
		out.push({ class: line?.class ?? '', style: line?.style, parts });
		start = end + 1;
	}
	return out;
}

/**
 * What a line names a note by: its subject, or the start of its text
 * without markup, as a list's mark, a heading's #s or the stars around
 * bold, so a name reads as words.
 */
export function noteTitle(note: { subject: string | null; body: string }) {
	if (note.subject) return note.subject;
	const line = note.body.split('\n').find((text) => text.trim()) ?? '';
	return line
		.replace(/^\s*(?:[-*+]\s+(?:\[[ xX]\]\s+)?|\d+[.)]\s+|#+\s+|>\s*)/, '')
		.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
		.replace(/\*\*|__|~~|`/g, '')
		.replace(/\*(\S[^*]*)\*/g, '$1')
		.trim();
}
