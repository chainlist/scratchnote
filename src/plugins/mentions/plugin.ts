import type { MarkdownConfig } from '@lezer/markdown';
import {
	ItemView,
	Plugin,
	Timeline,
	type App,
	type Note,
	type RenderedMarkdown
} from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';

/** Lucide's at-sign. */
const AT = '<circle cx="12" cy="12" r="4"/><path d="M16 8v5a3 3 0 0 0 6 0v-1a10 10 0 1 0-4 8"/>';
/** Lucide's file-text, a page's mark on the timeline, as the day has it. */
const FILE =
	'<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>';
/** The page of mentions, at `/plugin/mentions/`. */
const PAGE = 'mentions';
const AT_SIGN = 64;

/** A letter or digit of a name, in any script, or `_` and `-` inside one. */
const NAME = /[\p{L}\p{N}_-]/u;

/** `@name`, where the `@` does not follow a letter, so an email stays an email. */
const mentionSyntax: MarkdownConfig = {
	defineNodes: ['Mention'],
	parseInline: [
		{
			name: 'Mention',
			parse(cx, next, pos) {
				if (next !== AT_SIGN) return -1;
				if (pos > cx.offset && NAME.test(cx.slice(pos - 1, pos))) return -1;
				let end = pos + 1;
				while (end < cx.end && NAME.test(cx.slice(end, end + 1))) end++;
				// A name ends on a letter or digit: `@bob-` is `@bob` and a dash.
				while (end > pos + 1 && /[_-]/.test(cx.slice(end - 1, end))) end--;
				if (end === pos + 1) return -1;
				return cx.addElement(cx.elt('Mention', pos, end));
			},
			before: 'Link'
		}
	]
};

/** The names mentioned in a text, as the app parses it. */
function mentions(app: App, text: string): string[] {
	const found: string[] = [];
	app.markdown.parse(text).iterate({
		enter(node) {
			if (node.name === 'Mention') found.push(text.slice(node.from + 1, node.to));
		}
	});
	return found;
}

const same = (a: string, b: string) =>
	a.localeCompare(b, undefined, { sensitivity: 'accent' }) === 0;

function element<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	className?: string,
	text?: string
): HTMLElementTagNameMap[K] {
	const el = document.createElement(tag);
	if (className) el.className = className;
	if (text !== undefined) el.textContent = text;
	return el;
}

/**
 * Mentions, a core plugin (SPEC 3.10): `@name` as a chip, which opens the
 * notes that mention the name, and a page of every name mentioned.
 */
export class MentionsPlugin extends Plugin {
	onload() {
		const { app } = this;
		this.registerMarkdownSyntax({
			extension: mentionSyntax,
			render: {
				Mention: {
					// A chip: on a card it opens the notes that mention the name.
					widget(node) {
						const name = node.text.slice(1);
						if (node.where !== 'note') return element('span', 'mention', node.text);
						const chip = element('button', 'mention', node.text);
						chip.type = 'button';
						chip.title = m.mentions_chip_title({ name: node.text });
						chip.addEventListener('click', () => app.workspace.openPage(PAGE, { name }));
						return chip;
					}
				}
			}
		});
		this.registerPage(PAGE, () => new MentionsView());
		this.addCommand({
			id: 'all',
			name: m.mentions_show_all,
			icon: AT,
			callback: () => app.workspace.openPage(PAGE)
		});
	}
}

/** Every name mentioned, or the notes that mention one: `?name=bob`. */
class MentionsView extends ItemView {
	#count = 0;
	#drawn: RenderedMarkdown[] = [];
	#timeline: Timeline | null = null;
	#off: (() => void) | null = null;

	getDisplayText() {
		return this.params.name ? `@${this.params.name}` : m.mentions_plugin_name();
	}

	getIcon() {
		return AT;
	}

	getDetail() {
		return this.#count ? String(this.#count) : undefined;
	}

	async onOpen() {
		this.#off = this.app.on('notes-changed', () => void this.#draw());
		await this.#draw();
	}

	onClose() {
		this.#off?.();
		this.#clear();
	}

	#clear() {
		for (const rendered of this.#drawn) rendered.destroy();
		this.#drawn = [];
		this.#timeline?.clear();
		this.#timeline = null;
		this.containerEl.replaceChildren();
	}

	async #draw() {
		const { app } = this;
		const name = this.params.name;
		// Every `@`, since the backend matches case and `@Marie` is `@marie`.
		const notes = await app.notes.containing(['@']);
		this.#clear();
		if (name) this.#drawNotes(notes, name);
		else this.#drawNames(notes);
		this.refreshHeader();
	}

	/** Every name, most mentioned first, with how many notes mention it. */
	#drawNames(notes: Note[]) {
		const { app } = this;
		const counts = new Map<string, number>();
		for (const note of notes) {
			for (const found of new Set(mentions(app, note.body))) {
				const known = [...counts.keys()].find((other) => same(other, found)) ?? found;
				counts.set(known, (counts.get(known) ?? 0) + 1);
			}
		}
		this.#count = counts.size;
		if (counts.size === 0) {
			this.containerEl.append(element('p', 'mentions-empty', m.mentions_none()));
			return;
		}
		const list = element('ul', 'mentions-names');
		for (const [found, count] of [...counts].sort((a, b) => b[1] - a[1])) {
			const item = element('li');
			const button = element('button', 'mention', `@${found}`);
			button.type = 'button';
			button.addEventListener('click', () => app.workspace.openPage(PAGE, { name: found }));
			item.append(button, element('span', 'mentions-count', String(count)));
			list.append(item);
		}
		this.containerEl.append(list);
	}

	/** The notes that mention `name`, on the app's timeline. */
	#drawNotes(notes: Note[], name: string) {
		const { app } = this;
		const mentioning = notes.filter((note) =>
			mentions(app, note.body).some((found) => same(found, name))
		);
		this.#count = mentioning.length;
		if (mentioning.length === 0) {
			this.containerEl.append(
				element('p', 'mentions-empty', m.mentions_nothing({ name: `@${name}` }))
			);
			return;
		}
		const timeline = new Timeline(this.containerEl);
		this.#timeline = timeline;
		// Drawn as the day's timeline is; each item's body is ours to fill.
		for (const note of mentioning) {
			const item = timeline.addItem((item) =>
				item
					.setDate(note.date)
					.setTime(note.time)
					.setIcon(note.kind === 'page' ? FILE : undefined)
					.onClickTime(() => app.workspace.openNote(note))
			);
			if (note.kind === 'page' && note.subject)
				item.contentEl.append(element('p', 'mentions-title', note.subject));
			const text = element('div');
			item.contentEl.append(text);
			this.#drawn.push(app.markdown.render(text, note.body, { class: 'mentions-text' }));
		}
	}
}
