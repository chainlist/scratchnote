import { Autolink, Strikethrough, parser as commonmark } from '@lezer/markdown';
import type { SyntaxNode, Tree } from '@lezer/common';

/**
 * The markdown notes are shown with: CommonMark, bare links and
 * ~~strikethrough~~. The editor and the read-only view both go through
 * `preview`, so a note looks the same written and read.
 */
export const syntax = [Strikethrough, Autolink];
export const parser = commonmark.configure(syntax);

export type Preview = {
	/** Styled stretches; a link carries where it opens. */
	marks: { from: number; to: number; class: string; href?: string }[];
	/** Markup hidden while the cursor is off its line: `**`, `](url)`, `# `. A bullet stands in for a list mark. */
	hidden: { from: number; to: number; bullet?: boolean }[];
	/** Classes and styles for whole lines, by the offset each line starts at. */
	lines: Map<number, { class: string; style?: string }>;
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

export function preview(tree: Tree, text: string): Preview {
	const marks: Preview['marks'] = [];
	const hidden: Preview['hidden'] = [];
	const lines: Preview['lines'] = new Map();

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
					hidden.push({ from: start, to: end, bullet: true });
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
					marks.push({
						from: from + 1,
						to: close.from,
						class: 'md-link',
						href: href(text.slice(url.from, url.to))
					});
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
				case 'Image':
					return false;
			}
		}
	});

	return { marks, hidden, lines };
}

export type Part = { text: string; class: string; href?: string } | { bullet: true };
export type Line = { class: string; style?: string; parts: Part[] };

/** The note as lines of styled text with its markup hidden, for reading. */
export function renderLines(text: string): Line[] {
	const { marks, hidden, lines } = preview(parser.parse(text), text);

	const classes = new Array<string>(text.length).fill('');
	const hrefs = new Array<string | undefined>(text.length);
	for (const mark of marks) {
		for (let i = mark.from; i < mark.to; i++) {
			classes[i] += ` ${mark.class}`;
			if (mark.href) hrefs[i] = mark.href;
		}
	}
	// 1 hides a character, 2 draws a bullet in its place.
	const skip = new Uint8Array(text.length);
	for (const range of hidden) {
		skip.fill(1, range.from, range.to);
		if (range.bullet) skip[range.from] = 2;
	}

	const out: Line[] = [];
	let start = 0;
	for (const raw of text.split('\n')) {
		const end = start + raw.length;
		const parts: Part[] = [];
		for (let i = start; i < end;) {
			if (skip[i]) {
				if (skip[i] === 2) parts.push({ bullet: true });
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
