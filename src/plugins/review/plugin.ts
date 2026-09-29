import { ItemView, Plugin, iconSvg, type Note, type RenderedMarkdown } from '$lib/plugins/api';
import { m } from '$lib/paraglide/messages';
import { tasks, toggleTask } from '../basics/tasks/tasks';

/** Lucide's calendar-range. */
const WEEK =
	'<rect x="3" y="3" width="18" height="18" rx="2"/><path d="M16 2v3"/><path d="M3 9h18"/><path d="M8 2v3"/><path d="M17 13h-6"/><path d="M13 17H7"/><path d="M7 13h.01"/><path d="M17 17h.01"/>';
/** Lucide's file-text, a page's mark, as the day has it. */
const FILE =
	'<path d="M6 22a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h8a2.4 2.4 0 0 1 1.704.706l3.588 3.588A2.4 2.4 0 0 1 20 8v12a2 2 0 0 1-2 2z"/><path d="M14 2v5a1 1 0 0 0 1 1h5"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>';
/** Lucide's chevron-left and chevron-right. */
const PREVIOUS = '<path d="m15 18-6-6 6-6"/>';
const NEXT = '<path d="m9 18 6-6-6-6"/>';
/** The review, at `/plugin/review/`, or `?date=2026-09-29` to open the week holding a day. */
const PAGE = 'review';

/** The day `days` after `date`, by the calendar, so a clock change never skips one. */
function after(date: Date, days: number): Date {
	const day = new Date(date);
	day.setDate(day.getDate() + days);
	return day;
}

/** A day as the app names it, `2026-09-29`, in the local timezone. */
function iso(date: Date): string {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

/** The Monday of the week `date` is in. */
const mondayOf = (date: Date) => after(date, -((date.getDay() + 6) % 7));

/** The week's number, as ISO 8601 counts them: week 1 holds the year's first Thursday. */
function weekNumber(monday: Date): number {
	const thursday = after(monday, 3);
	const first = new Date(thursday.getFullYear(), 0, 1);
	const days = Math.round((thursday.getTime() - first.getTime()) / 86_400_000);
	return Math.floor(days / 7) + 1;
}

/** As the index counts them: runs of text between spaces with a letter or a digit, boxes left out. */
const words = (text: string) =>
	text.split(/\s+/).filter((token) => !/^\[[xX]\]$/.test(token) && /[\p{L}\p{N}]/u.test(token))
		.length;

/** A note the model has not named yet: its first line, without its list or heading mark. */
function firstLine(body: string): string {
	const line = body.split('\n').find((text) => text.trim()) ?? '';
	return line.replace(/^\s*(#+|>|[-*+]|\d+[.)])?\s*(\[[ xX]\]\s*)?/, '').trim();
}

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
 * Weekly review, a core plugin (SPEC 3.13): a week's notes by category, its
 * tasks, and the week in numbers, on a page of its own.
 */
export class ReviewPlugin extends Plugin {
	onload() {
		const open = () => this.app.workspace.openPage(PAGE);
		this.registerPage(PAGE, () => new ReviewView());
		this.addRibbonIcon(WEEK, m.review_plugin_name, open);
		this.addCommand({ id: 'open', name: m.review_open, icon: WEEK, callback: open });
	}
}

/** One week, Monday to Sunday: `?date=` names a day of it, the day shown otherwise. */
class ReviewView extends ItemView {
	#monday = mondayOf(new Date());
	#drawn: RenderedMarkdown[] = [];
	#off: (() => void) | null = null;
	/** Bodies with boxes clicked but not saved yet, drawn meanwhile. */
	#unsaved = new Map<string, string>();
	#saving = Promise.resolve();
	/** Counts the drawings, so one that a newer one overtook, or the close, draws nothing. */
	#drawing = 0;

	getDisplayText() {
		return m.review_week({ week: weekNumber(this.#monday) });
	}

	getIcon() {
		return WEEK;
	}

	getDetail() {
		const range = new Intl.DateTimeFormat(this.app.locale, {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		});
		return range.formatRange(this.#monday, after(this.#monday, 6));
	}

	async onOpen() {
		const day = this.params.date ?? this.app.workspace.day;
		if (/^\d{4}-\d{2}-\d{2}$/.test(day)) this.#monday = mondayOf(new Date(`${day}T00:00:00`));
		this.#off = this.app.on('notes-changed', () => void this.#draw());
		await this.#draw();
	}

	onClose() {
		this.#drawing++;
		this.#off?.();
		this.#clear();
	}

	#clear() {
		for (const rendered of this.#drawn) rendered.destroy();
		this.#drawn = [];
		this.containerEl.replaceChildren();
	}

	#go(weeks: number) {
		this.#monday = after(this.#monday, weeks * 7);
		void this.#draw();
	}

	async #draw() {
		const { app } = this;
		const drawing = ++this.#drawing;
		const dates = Array.from({ length: 7 }, (_, day) => iso(after(this.#monday, day)));
		const days = await app.notes.days();
		const written = new Set(days.map((day) => day.date));
		const notes = (
			await Promise.all(
				dates.filter((date) => written.has(date)).map((date) => app.notes.day(date))
			)
		)
			.flat()
			.filter((note) => !note.missing);
		if (drawing !== this.#drawing) return;

		this.#clear();
		const top = element('div', 'review-top');
		top.append(this.#numbers(notes), this.#nav(days.at(-1)?.date));
		this.containerEl.append(top);
		this.refreshHeader();

		if (notes.length === 0) {
			this.containerEl.append(element('p', 'review-empty', m.review_empty()));
			return;
		}
		this.#drawCategories(notes);
		this.#drawTasks(notes);
	}

	/** Notes, pages, words and days written. */
	#numbers(notes: Note[]): HTMLElement {
		const format = new Intl.NumberFormat(this.app.locale);
		const pages = notes.filter((note) => note.kind === 'page').length;
		const line = element('p', 'review-numbers');
		for (const [count, label] of [
			[notes.length - pages, m.review_notes],
			[pages, m.review_pages],
			[notes.reduce((sum, note) => sum + words(this.#body(note)), 0), m.review_words],
			[new Set(notes.map((note) => note.date)).size, m.review_days]
		] as const) {
			const figure = element('span', 'review-figure');
			figure.append(element('span', 'review-value', format.format(count)), ` ${label({ count })}`);
			line.append(figure);
		}
		return line;
	}

	/** The weeks before and after, from the first week written to this one. */
	#nav(oldest: string | undefined): HTMLElement {
		const nav = element('nav', 'review-nav');
		const arrow = (icon: string, title: string, weeks: number, disabled: boolean) => {
			const button = element('button', 'review-arrow');
			button.type = 'button';
			button.innerHTML = iconSvg(icon);
			button.title = title;
			button.disabled = disabled;
			button.addEventListener('click', () => this.#go(weeks));
			nav.append(button);
		};
		const monday = iso(this.#monday);
		arrow(PREVIOUS, m.review_previous(), -1, !oldest || oldest >= monday);
		arrow(NEXT, m.review_next(), 1, monday >= iso(mondayOf(new Date())));
		return nav;
	}

	/** A section per category, busiest first, a line per note; those without one last. */
	#drawCategories(notes: Note[]) {
		const { app } = this;
		const groups = new Map<string | null, Note[]>();
		for (const note of notes) {
			const group = groups.get(note.category);
			if (group) group.push(note);
			else groups.set(note.category, [note]);
		}
		const label = (category: string) => app.notes.categoryLabel(category);
		const sorted = [...groups].sort(([a, one], [b, other]) => {
			if (a === null || b === null) return a === null ? 1 : -1;
			return other.length - one.length || label(a).localeCompare(label(b), app.locale);
		});
		for (const [category, inCategory] of sorted) {
			const section = element('section', 'review-section');
			const heading = element(
				'h3',
				category === null ? 'review-heading' : 'review-heading review-category',
				category === null ? m.review_no_category() : `#${label(category)}`
			);
			heading.append(element('span', 'review-count', String(inCategory.length)));
			const list = element('ul', 'review-list');
			for (const note of inCategory) {
				const row = element('button', 'review-row');
				row.type = 'button';
				row.title = note.kind === 'page' ? m.pages_open() : m.tasks_show_in_day();
				row.addEventListener('click', () => app.workspace.openNote(note));
				if (note.kind === 'page') row.append(this.#fileIcon());
				if (note.subject) row.append(element('span', 'review-subject', note.subject));
				else {
					// Drawn as its card draws it, so its markup does not show.
					const line = element('span', 'review-subject review-unnamed');
					row.append(line);
					this.#drawn.push(
						app.markdown.render(line, firstLine(this.#body(note)), { preview: true })
					);
				}
				row.append(this.#when(note));
				const item = element('li');
				item.append(row);
				list.append(item);
			}
			section.append(heading, list);
			this.containerEl.append(section);
		}
	}

	/** The week's open tasks, which tick here, then the ticked ones. */
	#drawTasks(notes: Note[]) {
		const open = element('ul', 'review-list');
		const done = element('ul', 'review-list');
		for (const note of notes) {
			const body = this.#body(note);
			for (const task of tasks(this.app.markdown, body)) {
				const row = element('li', 'review-task');
				const text = element('div', 'review-task-text');
				const when = element('button', 'review-when-button');
				when.type = 'button';
				when.title = note.kind === 'page' ? m.pages_open() : m.tasks_show_in_day();
				when.addEventListener('click', () => this.app.workspace.openNote(note));
				if (note.kind === 'page') when.append(this.#fileIcon());
				when.append(this.#when(note));
				row.append(text, when);
				(task.done ? done : open).append(row);
				this.#drawn.push(
					this.app.markdown.render(text, body.slice(task.from, task.to), {
						onchange: () => this.#toggle(note, task.at),
						class: 'review-text'
					})
				);
			}
		}
		for (const [list, title] of [
			[open, m.review_tasks_open()],
			[done, m.review_tasks_done()]
		] as const) {
			if (list.childElementCount === 0) continue;
			const section = element('section', 'review-section');
			const heading = element('h3', 'review-heading', title);
			heading.append(element('span', 'review-count', String(list.childElementCount)));
			section.append(heading, list);
			this.containerEl.append(section);
		}
	}

	/** A note's text, with the boxes clicked here that are still saving. */
	#body(note: Note): string {
		return this.#unsaved.get(note.id) ?? note.body;
	}

	/** A box clicked saves at once; clicks in a row save one after another,
	 *  each on the text the one before left. */
	#toggle(note: Note, at: number) {
		const next = toggleTask(this.#body(note), at);
		this.#unsaved.set(note.id, next);
		this.#saving = this.#saving.then(async () => {
			let saved = true;
			try {
				// Only a box changed, so the note keeps its subject and category.
				await this.app.notes.setBody(note, next);
			} catch (e) {
				saved = false;
				console.error('review: a task was not saved', e);
			}
			if (this.#unsaved.get(note.id) === next) this.#unsaved.delete(note.id);
			// A save that failed changed nothing: the box goes back.
			if (!saved) await this.#draw();
		});
	}

	/** Its weekday and time, such as `Mon 09:12`. */
	#when(note: Note): HTMLElement {
		const weekday = new Intl.DateTimeFormat(this.app.locale, { weekday: 'short' }).format(
			new Date(`${note.date}T00:00:00`)
		);
		return element('span', 'review-when', `${weekday} ${note.time}`);
	}

	#fileIcon(): HTMLElement {
		const icon = element('span', 'review-icon');
		icon.innerHTML = iconSvg(FILE);
		return icon;
	}
}
