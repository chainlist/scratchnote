import { createContext, tick, type Snippet } from 'svelte';
import { goto, invalidate } from '$app/navigation';
import { resolve } from '$app/paths';
import { navigating, page } from '$app/state';
import {
	deleteNote,
	deletePage,
	isPage,
	noteToPage,
	retryEnrichment,
	updateNote,
	updateNoteMeta,
	type IndexEntry,
	type ModelStatus,
	type Note,
	type NoteEdit
} from '$lib/api';
import { categoryLabel } from '$lib/categories';
import { addToPageDraft } from '$lib/page-draft';
import { toggleCategory } from '$lib/query';

/** A view's title row, which the top bar shows a copy of once it scrolls away. */
export interface ViewTitle {
	/** Where the back arrow leads. Left out on a day, which has its own arrows. */
	back?: string;
	title?: string;
	/** After the title, dimmed: a search's whole query. */
	detail?: string;
	/** In place of the title, told whether it is the small copy in the top bar. */
	heading?: Snippet<[compact: boolean]>;
}

/**
 * What the main window's views share: the day they go back to, the docked
 * page, the dialogs, and what every card can do. The app layout makes one and
 * each view reaches it with `getShell`.
 */
export class Shell {
	/** The day last shown: the one the views go back to and a new page goes on. */
	day = $state('');
	error = $state<string | null>(null);
	model = $state<ModelStatus>({ state: 'absent' });
	canChat = $derived(this.model.state === 'loaded' || this.model.state === 'idle');
	/** Similar notes come from the embedding model's vectors. */
	embeddingInstalled = $state(false);
	canSimilar = $derived(this.embeddingInstalled && this.model.state !== 'disabled');
	/** The text size setting, in pixels: the rem the day's columns are sized in. */
	textSize = $state(16);
	/** The views' width, which a docked page narrows. Taken with the
	 *  scrollbar, which comes and goes with what the columns hold. */
	width = $state(0);

	/** The note open in the editor. */
	editing = $state<Note | null>(null);
	/** The note waiting on the delete confirmation. */
	deleting = $state<Note | null>(null);
	/** The note waiting on a title to become a page. */
	turning = $state<Note | null>(null);
	/** The page docked on the right, to write in while the day stays in reach. */
	docked = $state<Note | null>(null);
	/** The command center's query, kept between openings. */
	query = $state('');
	paletteOpen = $state(false);
	settingsOpen = $state(false);
	chatOpen = $state(false);
	/** The note a chat citation led to, blinking while it is set. */
	blinking = $state<string | null>(null);
	#blinkTimer: ReturnType<typeof setTimeout> | undefined;

	/** The view's title, for the top bar. */
	title = $state.raw<ViewTitle | null>(null);
	/** The view's title has scrolled under the top bar, which shows it instead. */
	titleCollapsed = $state(false);

	constructor(today: string) {
		this.day = today;
	}

	/** The day to go back to. */
	get back() {
		return resolve(`/day/${this.day}/`);
	}

	/** Load what the views show again, after a change here or on disk. */
	refresh = async () => {
		// An invalidation aborts a navigation under way, so it waits for one to land.
		while (navigating.complete) await navigating.complete.catch(() => {});
		await invalidate('app:notes');
		this.error = null;
	};

	showError = (message: string) => {
		this.error = message;
	};

	openDay = (date: string) => goto(resolve(`/day/${date}/`));

	/** Open a page in the views' place. A docked page leaves the dock, so no
	 *  page is open in two editors at once. */
	openPage = async (note: Pick<Note, 'id' | 'date'>) => {
		if (this.docked?.id === note.id) this.docked = null;
		await goto(resolve(`/page/${note.date}/${note.id}/`));
	};

	/** Start a page on the day shown. Its draft comes back if there is one;
	 *  an open one is already it. The focus stays for its title to take. */
	newPage = () => {
		if (this.draft) return;
		void goto(resolve(`/page/${this.day}/new/`), { keepFocus: true });
	};

	/** The new page open in the view, which takes text where it is typed. */
	draft: ((text: string) => void) | null = null;

	/** The capture window handed over its draft. It joins an open new page,
	 *  or starts one with whatever the page draft holds. */
	takeCaptureDraft = (body: string) => {
		if (this.draft) return this.draft(body);
		addToPageDraft(body);
		this.newPage();
	};

	showSimilar = (note: Note) => {
		void goto(resolve(`/similar/${note.date}/${note.id}/`));
	};

	showPages = () => goto(resolve('/pages/'));

	/** Every note a query matches, from the command center's "See all". */
	showResults = async (q: string) => {
		const trimmed = q.trim();
		this.query = trimmed;
		if (!trimmed) return this.openDay(this.day);
		// resolve() takes no query string, so the query follows the path it gives.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		await goto(`${resolve('/search/')}?q=${encodeURIComponent(trimmed)}`);
	};

	/** Open a note's day, bring the note into view and blink it. A page opens. */
	openCited = async (entry: Pick<IndexEntry, 'id' | 'date' | 'kind'>) => {
		if (isPage(entry)) return this.openPage(entry);
		await this.openDay(entry.date);
		// Cleared first so a second click on the same note blinks it again.
		this.blinking = null;
		await tick();
		this.blinking = entry.id;
		await tick();
		document
			.querySelector(`[data-note-id="${CSS.escape(entry.id)}"]`)
			?.scrollIntoView({ block: 'center', behavior: 'smooth' });
		clearTimeout(this.#blinkTimer);
		// A little past the animation, which runs 1.2s.
		this.#blinkTimer = setTimeout(() => (this.blinking = null), 1400);
	};

	/**
	 * A category clicked on a card opens the command center filtered on it.
	 * While the view lists results, it narrows them instead, see
	 * `toggleCategory`.
	 */
	openCategory = (category: string) => {
		if (page.route.id !== '/(app)/search') {
			this.query = `#${categoryLabel(category)} `;
			this.paletteOpen = true;
			return;
		}
		const q = page.url.searchParams.get('q') ?? '';
		void this.showResults(toggleCategory(q, categoryLabel(category)));
	};

	/** The inline editor: the body alone, which leaves the labels to the model. */
	saveBody = async (note: Note, body: string): Promise<boolean> => {
		try {
			if (body.trim() !== note.body) await updateNote(note.date, note.id, body);
			this.error = null;
			await this.refresh();
			return true;
		} catch (e) {
			this.error = String(e);
			return false;
		}
	};

	/** The full editor. Resolves to an error for it to show, or null once saved. */
	saveEdit = async (note: Note, edit: NoteEdit): Promise<string | null> => {
		const bodyChanged = edit.body.trim() !== note.body;
		// Only a real change to subject or category takes the note away from
		// the model, so an edit to the body alone leaves it to be re-enriched.
		const metaChanged =
			edit.subject.trim() !== (note.subject ?? '') || edit.category !== (note.category ?? '');
		try {
			if (bodyChanged) await updateNote(note.date, note.id, edit.body);
			if (metaChanged)
				await updateNoteMeta(note.date, note.id, {
					subject: edit.subject,
					category: edit.category
				});
			this.error = null;
			await this.refresh();
			return null;
		} catch (e) {
			await this.refresh();
			return String(e);
		}
	};

	/** Resolves to an error for the dialog to show, or null once done. */
	turnIntoPage = async (note: Note, title: string): Promise<string | null> => {
		try {
			const created = await noteToPage(note.date, note.id, title);
			this.error = null;
			await this.refresh();
			await this.openPage(created);
			return null;
		} catch (e) {
			return String(e);
		}
	};

	retry = async (note: Note) => {
		try {
			await retryEnrichment(note.date, note.id);
		} catch (e) {
			this.error = String(e);
		}
	};

	remove = async (note: Note) => {
		try {
			if (isPage(note)) await deletePage(note.date, note.id);
			else await deleteNote(note.date, note.id);
			if (this.editing?.id === note.id) this.editing = null;
			if (this.docked?.id === note.id) this.docked = null;
			// The page itself, or the notes similar to it, go back to the day.
			if (page.params.id === note.id) await this.openDay(this.day);
			await this.refresh();
		} catch (e) {
			this.error = String(e);
		}
	};

	/**
	 * Another space has its own days and notes, so a search, similar notes or
	 * a page from the last one would mean nothing there. Its categories and
	 * days are listed afresh.
	 */
	switchSpace = async () => {
		this.query = '';
		this.docked = null;
		const route = page.route.id;
		if (
			route === '/(app)/search' ||
			route === '/(app)/similar/[date]/[id]' ||
			route === '/(app)/page/[date]/[id]'
		)
			await this.openDay(this.day);
		await this.refresh();
	};

	/** What every card can do. */
	cardActions = $derived({
		onedit: (note: Note) => (this.editing = note),
		ondelete: (note: Note) => (this.deleting = note),
		onsave: this.saveBody,
		onretry: this.retry,
		oncategory: this.openCategory,
		onsimilar: this.canSimilar ? this.showSimilar : undefined,
		onopen: (note: Note) => void this.openPage(note),
		ondock: (note: Note) => (this.docked = note),
		onpage: (note: Note) => (this.turning = note),
		onerror: this.showError
	});
}

export const [getShell, setShell] = createContext<Shell>();
