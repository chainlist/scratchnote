import { Autolink, Strikethrough, TaskList, parser as commonmark } from '@lezer/markdown';
import type { SyntaxNode, Tree } from '@lezer/common';

/**
 * The markdown notes are shown with: CommonMark, bare links,
 * ~~strikethrough~~ and `- [ ]` tasks. The editor and the read-only view both go through
 * `preview`, so a note looks the same written and read.
 */
export const syntax = [Strikethrough, Autolink, TaskList];
export const parser = commonmark.configure(syntax);

export type Preview = {
	/** Styled stretches; a link carries where it opens. */
	marks: { from: number; to: number; class: string; href?: string }[];
	/**
	 * Markup hidden while the cursor is off its line: `**`, `](url)`, `# `.
	 * A bullet stands in for a list mark, and a checkbox for a task's `[ ]`.
	 */
	hidden: { from: number; to: number; bullet?: boolean; task?: { done: boolean } }[];
	/** Classes and styles for whole lines, by the offset each line starts at. */
	lines: Map<number, { class: string; style?: string }>;
	/**
	 * Links to attached files, each drawn in place of its markup: the image
	 * for `![name](path)` to an image, a card for any other.
	 */
	attachments: { from: number; to: number; path: string; name: string; image: boolean }[];
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
	const marks: Preview['marks'] = [];
	const hidden: Preview['hidden'] = [];
	const lines: Preview['lines'] = new Map();
	const attachments: Preview['attachments'] = [];
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
					// A task's checkbox takes the bullet's place.
					const task = node.node.parent.getChild('Task') !== null;
					hidden.push({ from: start, to: end, bullet: !task });
					addLine(
						lineStart(from),
						'md-li',
						`padding-left: calc(var(--md-indent, 0em) + ${depth * BULLET_EM}em); text-indent: -${BULLET_EM}em`
					);
					break;
				}
				case 'TaskMarker': {
					let end = to;
					while (text[end] === ' ') end++;
					hidden.push({ from, to: end, task: { done: text[from + 1] !== ' ' } });
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
		}
	});

	return { marks, hidden, lines, attachments };
}

export type Part =
	| { text: string; class: string; href?: string }
	| { bullet: true }
	/** A task's checkbox, by the offset of its `[`. */
	| { task: number; done: boolean }
	| { attachment: string; name: string; image: boolean };
export type Line = { class: string; style?: string; parts: Part[] };

/** The note as lines of styled text with its markup hidden, for reading. */
export function renderLines(text: string): Line[] {
	const { marks, hidden, lines, attachments } = preview(parser.parse(text), text);

	const classes = new Array<string>(text.length).fill('');
	const hrefs = new Array<string | undefined>(text.length);
	for (const mark of marks) {
		for (let i = mark.from; i < mark.to; i++) {
			classes[i] += ` ${mark.class}`;
			if (mark.href) hrefs[i] = mark.href;
		}
	}
	// 1 hides a character, 2 draws a bullet in its place, 3 an attachment,
	// 4 a task's checkbox.
	const skip = new Uint8Array(text.length);
	for (const range of hidden) {
		skip.fill(1, range.from, range.to);
		if (range.bullet) skip[range.from] = 2;
		if (range.task) skip[range.from] = 4;
	}
	const attachmentAt = new Map(attachments.map((attached) => [attached.from, attached]));
	for (const attached of attachments) {
		skip.fill(1, attached.from, attached.to);
		skip[attached.from] = 3;
	}

	const out: Line[] = [];
	let start = 0;
	for (const raw of text.split('\n')) {
		const end = start + raw.length;
		const parts: Part[] = [];
		for (let i = start; i < end;) {
			if (skip[i]) {
				if (skip[i] === 2) parts.push({ bullet: true });
				if (skip[i] === 4) parts.push({ task: i, done: text[i + 1] !== ' ' });
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

/** The text with the task box whose `[` is at `at` ticked, or cleared if it was. */
export const toggleTask = (text: string, at: number) =>
	text.slice(0, at + 1) + (text[at + 1] === ' ' ? 'x' : ' ') + text.slice(at + 2);
