import type { SyntaxNode, Tree } from '@lezer/common';
import type { WidgetContext } from '#lib/plugins/types.js';
import { attachmentPath, isImage } from './attachments.js';
import { current } from './syntax.js';

/** What a plugin's widget draws itself from. */
export type WidgetRender = (node: WidgetContext) => HTMLElement;

type Preview = {
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
