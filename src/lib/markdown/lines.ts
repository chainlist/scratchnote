import { preview, type WidgetRender } from './preview.js';
import { parseMarkdown } from './syntax.js';

type Part =
	| { text: string; class: string; href?: string }
	| { bullet: true }
	/** A plugin's widget for the node `text` at `from`. */
	| { widget: WidgetRender; text: string; from: number }
	| { attachment: string; name: string; image: boolean };
type Line = { class: string; style?: string; parts: Part[] };

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
