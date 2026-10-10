import { Decoration, EditorView, WidgetType } from '@codemirror/view';
import { attachmentUrl } from '#lib/notes/attachments.svelte.js';
import { cardName, fileName, fileType, type WidgetRender } from '#lib/markdown.js';

/** A list item's bullet, drawn in place of its marker. */
class Bullet extends WidgetType {
	eq() {
		return true;
	}
	toDOM() {
		const span = document.createElement('span');
		span.className = 'md-bullet';
		return span;
	}
}
export const bullet = Decoration.replace({ widget: new Bullet() });

/** What a plugin draws for one of its nodes, such as a task's box (SPEC 3.9). */
export class PluginWidget extends WidgetType {
	readonly render: WidgetRender;
	readonly text: string;
	readonly clicks: boolean;
	constructor(render: WidgetRender, text: string, clicks: boolean) {
		super();
		this.render = render;
		this.text = text;
		this.clicks = clicks;
	}
	eq(other: PluginWidget) {
		return other.render === this.render && other.text === this.text;
	}
	toDOM(view: EditorView) {
		const text = this.text;
		let dom: HTMLElement | undefined;
		try {
			dom = this.render({
				text,
				where: 'editor',
				editable: true,
				// Where the node is now, which edits elsewhere may have moved.
				update: (next) => {
					if (!dom) return;
					const from = view.posAtDOM(dom);
					view.dispatch({ changes: { from, to: from + text.length, insert: next } });
				}
			});
		} catch (e) {
			// A faulty plugin leaves the text as typed.
			console.error(e);
			dom = element('span', '', text);
		}
		return dom;
	}
	// A widget that handles its own clicks keeps them from the editor; any
	// other takes the cursor there, which brings its markup back to edit.
	ignoreEvent() {
		return this.clicks;
	}
}

export function element(tag: string, className: string, text = '') {
	const node = document.createElement(tag);
	node.className = className;
	node.textContent = text;
	return node;
}

/** An attachment drawn where its markup is: its image, or the card the read-only view draws. */
export class Attached extends WidgetType {
	readonly path: string;
	readonly name: string;
	readonly image: boolean;
	readonly space: string | undefined;
	constructor(path: string, name: string, image: boolean, space: string | undefined) {
		super();
		this.path = path;
		this.name = name || fileName(path);
		this.image = image;
		this.space = space;
	}
	eq(other: Attached) {
		return (
			other.path === this.path &&
			other.name === this.name &&
			other.image === this.image &&
			other.space === this.space
		);
	}
	toDOM(view: EditorView) {
		if (this.image) {
			const img = document.createElement('img');
			img.className = 'md-image';
			img.src = attachmentUrl(this.path, this.space);
			img.alt = this.name;
			img.dataset.attachment = this.path;
			// Its height is known once it loads, and the editor's lines move for it.
			img.onload = () => view.requestMeasure();
			return img;
		}
		const card = element('span', 'md-file');
		card.title = this.name;
		card.dataset.attachment = this.path;
		card.append(
			element('span', 'md-file-type', fileType(this.path)),
			element('span', 'md-file-name', cardName(this.name, this.path))
		);
		return card;
	}
	// A click on it puts the cursor there, which brings its markup back to edit.
	ignoreEvent() {
		return false;
	}
}
