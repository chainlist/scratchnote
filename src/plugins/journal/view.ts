import { ItemView, iconSvg, type Note, type RenderedMarkdown } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import { element } from '../dom';
import { random, stains } from './paper';
import { photo, settle, withoutImages } from './photos';
import { animateTurn } from './turn';

/** Lucide's book-open. */
export const BOOK =
	'<path d="M12 7v14"/><path d="M3 18a1 1 0 0 1-1-1V4a1 1 0 0 1 1-1h5a4 4 0 0 1 4 4 4 4 0 0 1 4-4h5a1 1 0 0 1 1 1v13a1 1 0 0 1-1 1h-6a3 3 0 0 0-3 3 3 3 0 0 0-3-3z"/>';
/** Lucide's chevron-left and chevron-right. */
const PREVIOUS = '<path d="m15 18-6-6 6-6"/>';
const NEXT = '<path d="m9 18 6-6-6-6"/>';

/**
 * A day's notes laid out on the two pages of a spread: the text runs in
 * columns, two to a spread, so a day too long for one carries on past it,
 * off to the right, and `sheet` says which pair shows.
 */
interface Spread {
	el: HTMLElement;
	flow: HTMLElement;
	/** The last thing in the flow, whose column says how many spreads the day takes. */
	end: HTMLElement;
	/** The left page's and the right page's stains, running head and page number. */
	stains: [HTMLElement, HTMLElement];
	heads: [HTMLElement, HTMLElement];
	folios: [HTMLElement, HTMLElement];
	drawn: RenderedMarkdown[];
	/** Null for the empty book. */
	date: string | null;
	notes: Note[];
	sheets: number;
	sheet: number;
}

/** A day as the app names it, `2026-09-29`, read in the local timezone. */
const localDate = (date: string) => new Date(`${date}T00:00:00`);

/** How many spreads the day takes: the column its end falls in, two to a spread. */
function measure(spread: Spread) {
	const { flow, end } = spread;
	const gap = parseFloat(getComputedStyle(flow).columnGap) || 0;
	const pitch = (flow.offsetWidth + gap) / 2;
	if (!pitch) return;
	const column = Math.round(
		(end.getBoundingClientRect().left - flow.getBoundingClientRect().left) / pitch
	);
	spread.sheets = Math.max(1, Math.floor(column / 2) + 1);
}

/** The book: the days with notes, oldest first, one to a spread. */
export class JournalView extends ItemView {
	#days: string[] = [];
	/** The day open, in `#days`. */
	#index = 0;
	#spread: Spread | null = null;
	/** Every spread drawn and not yet destroyed: the one showing, and one being turned to. */
	#spreads = new Set<Spread>();
	/** A turn or a redraw under way. */
	#busy = false;
	/** A turn asked for during another, taken after it: true for forward. */
	#queued: boolean | null = null;
	/** The notes changed during a turn: draw them after it. */
	#stale = false;
	#closed = false;
	#cleanups: (() => void)[] = [];

	#book!: HTMLElement;
	#pages!: HTMLElement;
	#nav!: HTMLElement;
	#previous!: HTMLButtonElement;
	#next!: HTMLButtonElement;
	#when!: HTMLButtonElement;
	#cornerPrevious!: HTMLButtonElement;
	#cornerNext!: HTMLButtonElement;

	getDisplayText() {
		return m.journal_title();
	}

	getIcon() {
		return BOOK;
	}

	async onOpen() {
		this.#build();
		const onKey = (event: KeyboardEvent) => this.#key(event);
		window.addEventListener('keydown', onKey);
		// A new width lays the text out anew, and the day may take more spreads or fewer.
		const resize = new ResizeObserver(() => this.#fit());
		resize.observe(this.#pages);
		this.#cleanups.push(
			this.app.on('notes-changed', () => this.#changed()),
			() => window.removeEventListener('keydown', onKey),
			() => resize.disconnect()
		);
		await this.#run(() => this.#open(this.params.date ?? this.app.workspace.day, 0));
	}

	onClose() {
		this.#closed = true;
		for (const cleanup of this.#cleanups.splice(0)) cleanup();
		for (const spread of [...this.#spreads]) this.#destroy(spread);
		this.containerEl.replaceChildren();
	}

	#build() {
		const root = element('div', 'journal');
		this.#book = element('div', 'journal-book');
		this.#pages = element('div', 'journal-pages');
		this.#cornerPrevious = this.#button('journal-corner journal-corner-previous', () =>
			this.#go(false)
		);
		this.#cornerNext = this.#button('journal-corner journal-corner-next', () => this.#go(true));
		this.#cornerPrevious.setAttribute('aria-label', m.journal_previous());
		this.#cornerNext.setAttribute('aria-label', m.journal_next());
		this.#pages.append(this.#cornerPrevious, this.#cornerNext);
		this.#book.append(
			element('div', 'journal-stack journal-stack-left'),
			element('div', 'journal-stack journal-stack-right'),
			this.#pages,
			element('div', 'journal-bookmark')
		);

		this.#nav = element('nav', 'journal-nav');
		this.#previous = this.#button('journal-arrow', () => this.#go(false));
		this.#previous.innerHTML = iconSvg(PREVIOUS);
		this.#previous.title = m.journal_previous();
		this.#next = this.#button('journal-arrow', () => this.#go(true));
		this.#next.innerHTML = iconSvg(NEXT);
		this.#next.title = m.journal_next();
		this.#when = this.#button('journal-when', () => {
			if (this.#spread?.date) this.app.workspace.openDay(this.#spread.date);
		});
		this.#when.title = m.journal_open_day();
		this.#nav.append(this.#previous, this.#when, this.#next);

		root.append(this.#book, this.#nav);
		this.containerEl.replaceChildren(root);
	}

	#button(className: string, onclick: () => void): HTMLButtonElement {
		const button = element('button', className);
		button.type = 'button';
		button.addEventListener('click', onclick);
		return button;
	}

	/**
	 * Run a turn or a redraw, one at a time. A turn asked for meanwhile
	 * waits its go, the notes having changed goes first.
	 */
	async #run(task: () => Promise<void>) {
		this.#busy = true;
		try {
			await task();
		} catch (e) {
			console.error('journal:', e);
		} finally {
			this.#busy = false;
		}
		if (this.#closed) return;
		if (this.#stale) {
			this.#stale = false;
			const spread = this.#spread;
			await this.#run(() => this.#open(spread?.date ?? this.app.workspace.day, spread?.sheet ?? 0));
		} else if (this.#queued !== null) {
			const forward = this.#queued;
			this.#queued = null;
			this.#go(forward);
		}
	}

	#changed() {
		if (this.#busy) this.#stale = true;
		else
			void this.#run(() =>
				this.#open(this.#spread?.date ?? this.app.workspace.day, this.#spread?.sheet ?? 0)
			);
	}

	/** Show `date` at `sheet`, or the last day written before it at its start, without a turn. */
	async #open(date: string, sheet: number) {
		const days = (await this.app.notes.days())
			.filter((day) => day.count > 0)
			.map((day) => day.date)
			.sort();
		if (this.#closed) return;
		this.#days = days;
		const found = days.indexOf(date);
		this.#index =
			found >= 0
				? found
				: Math.max(
						days.findLastIndex((day) => day < date),
						0
					);
		const spread = await this.#prepare(days[this.#index] ?? null, found >= 0 ? sheet : 0);
		if (spread) this.#swap(spread);
	}

	/** Turn a page forward or back: the rest of a long day, or the next day or the one before. */
	#go(forward: boolean) {
		if (this.#closed) return;
		if (this.#busy) {
			this.#queued = forward;
			return;
		}
		const spread = this.#spread;
		if (!spread?.date) return;
		const { date, notes, sheet, sheets } = spread;
		const index = this.#index + (forward ? 1 : -1);
		if (forward && sheet < sheets - 1) {
			void this.#run(() => this.#turn(this.#index, sheet + 1, notes, true));
		} else if (!forward && sheet > 0) {
			void this.#run(() => this.#turn(this.#index, sheet - 1, notes, false));
		} else if (index >= 0 && index < this.#days.length && this.#days[this.#index] === date) {
			void this.#run(() => this.#turn(index, forward ? 0 : 'last', undefined, forward));
		}
	}

	#key(event: KeyboardEvent) {
		if (event.defaultPrevented || event.altKey || event.ctrlKey || event.metaKey || event.shiftKey)
			return;
		const target = event.target as HTMLElement;
		if (target.isContentEditable || target.closest('input, textarea, select, [role="dialog"]'))
			return;
		if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
		event.preventDefault();
		this.#go(event.key === 'ArrowRight');
	}

	/**
	 * Draw a day's spread under the one showing, wait for its photos, and
	 * place it at `sheet`. Null when the view closed meanwhile.
	 */
	async #prepare(
		date: string | null,
		sheet: number | 'last',
		notes?: Note[]
	): Promise<Spread | null> {
		const dayNotes = notes ?? (date ? await this.app.notes.day(date) : []);
		if (this.#closed) return null;
		const spread = this.#draw(date, dayNotes);
		this.#pages.prepend(spread.el);
		await settle(spread.flow);
		if (this.#closed) {
			this.#destroy(spread);
			return null;
		}
		measure(spread);
		this.#place(spread, sheet === 'last' ? spread.sheets - 1 : Math.min(sheet, spread.sheets - 1));
		return spread;
	}

	#draw(date: string | null, notes: Note[]): Spread {
		const el = element('div', 'journal-spread');
		const papers = [
			element('div', 'journal-paper journal-left'),
			element('div', 'journal-paper journal-right')
		];
		const stains: Spread['stains'] = [
			element('div', 'journal-stains'),
			element('div', 'journal-stains')
		];
		papers[0].append(stains[0]);
		papers[1].append(stains[1]);
		const heads: Spread['heads'] = [
			element('span', 'journal-head journal-left'),
			element('span', 'journal-head journal-right')
		];
		const folios: Spread['folios'] = [
			element('span', 'journal-folio journal-left'),
			element('span', 'journal-folio journal-right')
		];
		const frame = element('div', 'journal-frame');
		const flow = element('div', 'journal-flow');
		frame.append(flow);
		el.append(...papers, frame, ...heads, ...folios);

		const drawn: RenderedMarkdown[] = [];
		if (!date) {
			flow.append(element('p', 'journal-empty', m.journal_empty()));
		} else {
			const header = element('header', 'journal-date');
			const day = localDate(date);
			header.append(
				element(
					'span',
					'journal-weekday',
					day.toLocaleDateString(this.app.locale, { weekday: 'long' })
				),
				element(
					'span',
					'journal-day',
					day.toLocaleDateString(this.app.locale, {
						day: 'numeric',
						month: 'long',
						year: 'numeric'
					})
				)
			);
			flow.append(header);
			notes.forEach((note, i) => {
				if (i > 0) flow.append(element('div', 'journal-divider'));
				flow.append(this.#note(note, drawn));
			});
		}
		const end = element('div', 'journal-end');
		flow.append(end);

		const spread: Spread = {
			el,
			flow,
			end,
			stains,
			heads,
			folios,
			drawn,
			date,
			notes,
			sheets: 1,
			sheet: 0
		};
		this.#spreads.add(spread);
		return spread;
	}

	/**
	 * A note on the page: its time, its text, and its pictures stuck in.
	 * The first one often goes beside the text, which wraps round it, on
	 * either side; the others under it.
	 */
	#note(note: Note, drawn: RenderedMarkdown[]): HTMLElement {
		const { app } = this;
		const article = element('article', 'journal-note');
		const time = this.#button('journal-time', () => app.workspace.openNote(note));
		time.textContent = note.time;
		// The time names it; this says, on hover and read after it, where it leads.
		time.title = m.journal_open_day();
		article.append(time);
		if (note.kind === 'page' && note.subject)
			article.append(element('h3', 'journal-title', note.subject));
		const images = app.markdown.images(note.body);
		const text = withoutImages(note.body, images);
		const rand = random(`${note.id}:layout`);
		const beside =
			text && images.length && rand() < 0.65 ? (rand() < 0.5 ? 'left' : 'right') : null;
		if (beside) article.append(photo(images[0], beside));
		if (text) {
			const body = element('div');
			article.append(body);
			drawn.push(app.markdown.render(body, text, { class: 'journal-text' }));
		}
		const under = beside ? images.slice(1) : images;
		if (under.length) {
			const photos = element('div', 'journal-photos');
			for (const image of under) photos.append(photo(image, under.length === 1 ? 'alone' : 'row'));
			article.append(photos);
		}
		return article;
	}

	/** Show the spread's `sheet`: its columns, its running heads and page numbers, its stains. */
	#place(spread: Spread, sheet: number) {
		spread.sheet = sheet;
		spread.flow.style.setProperty('--sheet', String(sheet));
		const { date } = spread;
		const continued =
			date && sheet > 0
				? m.journal_continued({
						date: localDate(date).toLocaleDateString(this.app.locale, {
							day: 'numeric',
							month: 'long'
						})
					})
				: '';
		spread.heads[0].textContent = continued;
		spread.folios[0].textContent = date ? String(sheet * 2 + 1) : '';
		spread.folios[1].textContent = date ? String(sheet * 2 + 2) : '';
		const seed = `${date ?? 'empty'}:${sheet}`;
		spread.stains[0].style.backgroundImage = stains(`${seed}:left`);
		spread.stains[1].style.backgroundImage = stains(`${seed}:right`);
	}

	/** Turn a page to the spread of the day at `index`, drawn first under the one showing. */
	async #turn(index: number, sheet: number | 'last', notes: Note[] | undefined, forward: boolean) {
		const current = this.#spread;
		const target = await this.#prepare(this.#days[index], sheet, notes);
		if (!target) return;
		if (current && !matchMedia('(prefers-reduced-motion: reduce)').matches) {
			const closed = () => this.#closed;
			if (!(await animateTurn(this.#pages, current.el, target.el, forward, closed))) return;
		}
		this.#index = index;
		this.#swap(target);
	}

	/** The target spread, under the one showing, takes its place. */
	#swap(next: Spread) {
		const current = this.#spread;
		this.#spread = next;
		if (current && current !== next) this.#destroy(current);
		this.#update();
	}

	#destroy(spread: Spread) {
		for (const rendered of spread.drawn) rendered.destroy();
		spread.el.remove();
		this.#spreads.delete(spread);
	}

	#fit() {
		const spread = this.#spread;
		if (!spread || this.#busy) return;
		measure(spread);
		if (spread.sheet >= spread.sheets) this.#place(spread, spread.sheets - 1);
		this.#update();
	}

	/** The way forward and back, the date under the book, and how thick it is each side. */
	#update() {
		const spread = this.#spread;
		if (!spread) return;
		const back = !!spread.date && (spread.sheet > 0 || this.#index > 0);
		const forward =
			!!spread.date && (spread.sheet < spread.sheets - 1 || this.#index < this.#days.length - 1);
		this.#previous.disabled = !back;
		this.#next.disabled = !forward;
		this.#cornerPrevious.hidden = !back;
		this.#cornerNext.hidden = !forward;
		this.#nav.hidden = !spread.date;
		this.#when.textContent = spread.date
			? localDate(spread.date).toLocaleDateString(this.app.locale, {
					weekday: 'long',
					day: 'numeric',
					month: 'long',
					year: 'numeric'
				})
			: '';
		const read = this.#days.length > 1 ? this.#index / (this.#days.length - 1) : 0;
		this.#book.style.setProperty('--read', read.toFixed(3));
	}
}
