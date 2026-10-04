import { createContext, tick, type Snippet } from 'svelte';
import { EditorView } from '@codemirror/view';
import { goto, invalidate } from '$app/navigation';
import { resolve } from '$app/paths';
import { navigating, page } from '$app/state';
import {
	clearDayAhead,
	deleteNote,
	deletePage,
	isPage,
	keepOutOfThreads,
	listThreads,
	moveNote,
	movePage,
	noteToPage,
	updateNote,
	type Note,
	type SpacesView,
	type Thread
} from '#lib/api.js';
import { loadDock, saveDock, type DockSide } from '#lib/dock.js';
import { addToPageDraft } from '#lib/page-draft.js';
import { notesChanged, type WorkspaceHost } from '#lib/plugins/app.js';

/** Where a note sits in its thread: the thread, and its place in it from 0. */
export interface ThreadPlace {
	thread: Thread;
	index: number;
}

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
 * each view reaches it with `getShell`. The plugins reach the views through
 * it too, as `app.workspace` (SPEC 3.9).
 */
export class Shell implements WorkspaceHost {
	/** The day last shown: the one the views go back to and a new page goes on. */
	day = $state('');
	error = $state<string | null>(null);
	/** Similar notes come from the embedding model's vectors. */
	embeddingInstalled = $state(false);
	canSimilar = $derived(this.embeddingInstalled);
	/** The thread each note of the open space is in, by note (SPEC 6.4). */
	threads = $state.raw<Record<string, ThreadPlace>>({});
	/** The notes taken out of threads. */
	alone = $state.raw<string[]>([]);
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
	/** The note or page waiting on the space it moves to. */
	moving = $state<Note | null>(null);
	/** Every space and the open one, as the app layout last listed them. */
	spaces = $state.raw<SpacesView | null>(null);
	/** A note can move only with another space to go to. */
	canMove = $derived((this.spaces?.spaces.length ?? 0) > 1);
	/** The page docked beside the view, to write in while the day stays in reach. */
	docked = $state<Note | null>(null);
	/** A plugin's view docked beside the view instead, by its type. */
	panel = $state<string | null>(null);
	dockOpen = $derived(this.docked !== null || this.panel !== null);
	#dockLayout = loadDock();
	/** The side of the view the dock sits on. */
	dockSide = $state<DockSide>(this.#dockLayout.side);
	/** The dock's width, as a percentage. Not reactive: the dock takes it as
	 *  it opens, and from there its pane keeps its own. */
	dockSize = this.#dockLayout.size;
	/** The editor the command center was opened from, for commands on its text. */
	paletteEditor = $state.raw<EditorView | null>(null);
	/** The command center's query, kept between openings. */
	query = $state('');
	paletteOpen = $state(false);
	settingsOpen = $state(false);
	/** The note a link to it led to, blinking while it is set. */
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
		return resolve(`day/${this.day}/`);
	}

	/** Load what the views show again, after a change here or on disk. */
	refresh = async () => {
		// An invalidation aborts a navigation under way, so it waits for one to land.
		while (navigating.complete) await navigating.complete.catch(() => {});
		await invalidate('app:notes');
		this.error = null;
		// The plugins' pages and panels load their notes themselves.
		notesChanged();
	};

	showError = (message: string) => {
		this.error = message;
	};

	openDay = (date: string) => goto(resolve(`day/${date}/`));

	/** Open a page in the views' place. A docked page leaves the dock, so no
	 *  page is open in two editors at once. */
	openPage = async (note: Pick<Note, 'id' | 'date'>) => {
		if (this.docked?.id === note.id) this.docked = null;
		await goto(resolve(`page/${note.date}/${note.id}/`));
	};

	/** Start a page on the day shown. Its draft comes back if there is one;
	 *  an open one is already it. The focus stays for its title to take. */
	newPage = () => {
		if (this.draft) return;
		void goto(resolve(`page/${this.day}/new/`), { reset: false });
	};

	/** The new page open in the view, which takes text where it is typed. */
	draft: ((text: string) => void) | null = null;

	/** The capture window handed over its draft. It joins an open new page,
	 *  or starts one with whatever the page draft holds. A draft for another
	 *  space comes after that space opens, so it waits for the views to leave
	 *  the last one, a new page open there included. */
	takeCaptureDraft = async (body: string) => {
		await this.switching.catch(() => {});
		if (this.draft) return this.draft(body);
		addToPageDraft(body);
		this.newPage();
	};

	showSimilar = (note: Note) => {
		void goto(resolve(`similar/${note.date}/${note.id}/`));
	};

	/** Where a note sits in its thread, while threads are on offer. */
	threadOf = (id: string): ThreadPlace | undefined =>
		this.canSimilar ? this.threads[id] : undefined;

	/** A note taken out of threads, which can be let back in while threads are on offer. */
	keptOut = (id: string) => this.canSimilar && this.alone.includes(id);

	/** Read the open space's threads again, as the backend placed them. */
	loadThreads = async () => {
		try {
			const view = await listThreads();
			const places: Record<string, ThreadPlace> = {};

			for (const thread of view.threads)
				thread.notes.forEach((id, index) => (places[id] = { thread, index }));

			this.threads = places;
			this.alone = view.alone;
			// The view of a thread lists its notes itself.
			await invalidate('app:threads');
		} catch (e) {
			this.error = String(e);
		}
	};

	showThread = (id: string) => goto(resolve(`thread/${id}/`));

	/** Forget the day ahead read in a note, for one read wrong. */
	clearDayAhead = async (note: Pick<Note, 'id' | 'date'>) => {
		try {
			await clearDayAhead(note.date, note.id);
			await this.refresh();
		} catch (e) {
			this.error = String(e);
		}
	};

	/** Take a note out of threads, where it stays, or let it back in. */
	keepOut = async (note: Pick<Note, 'id'>, out: boolean) => {
		try {
			await keepOutOfThreads(note.id, out);
		} catch (e) {
			this.error = String(e);
		}
	};

	showPages = () => goto(resolve('pages/'));

	/** A plugin's page, `/plugin/<type>/`, with its query string. */
	openPluginPage = (type: string, params: Record<string, string> = {}) => {
		const query = Object.entries(params)
			.map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(value)}`)
			.join('&');
		// resolve() takes no query string, so the query follows the path it gives.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		return goto(`${resolve(`plugin/${type}/`)}${query ? `?${query}` : ''}`);
	};

	/** The dock holds one thing: a page, or a plugin's view. */
	dock = (note: Note) => {
		this.panel = null;
		this.docked = note;
	};

	openPanel = (type: string) => {
		this.docked = null;
		this.panel = type;
	};

	closePanel = (type?: string) => {
		if (type === undefined || this.panel === type) this.panel = null;
	};

	/** The dock goes to the view's other side, as wide as it was. */
	flipDock = () => {
		this.dockSide = this.dockSide === 'left' ? 'right' : 'left';
		saveDock({ side: this.dockSide, size: this.dockSize });
	};

	resizeDock = (size: number) => {
		this.dockSize = size;
		saveDock({ side: this.dockSide, size });
	};

	/** A page open in the page view or the dock, which releases it when it closes. */
	isPageOpen = (id: string) => this.docked?.id === id || page.params.id === id;

	/** The command center, told which editor it was opened from. */
	openPalette = () => {
		const text = document.activeElement?.closest('.cm-editor');
		this.paletteEditor = text instanceof HTMLElement ? EditorView.findFromDOM(text) : null;
		this.paletteOpen = true;
	};

	/** Every note a query matches, from the command center's "See all". */
	showResults = async (q: string) => {
		const trimmed = q.trim();
		this.query = trimmed;
		if (!trimmed) return this.openDay(this.day);
		// resolve() takes no query string, so the query follows the path it gives.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		await goto(`${resolve('search/')}?q=${encodeURIComponent(trimmed)}`);
	};

	/** Open a note's day, bring the note into view and blink it. A page opens. */
	openCited = async (entry: Pick<Note, 'id' | 'date' | 'kind'>) => {
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

	/** The inline editor. */
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
	saveEdit = async (note: Note, body: string): Promise<string | null> => {
		try {
			if (body.trim() !== note.body) await updateNote(note.date, note.id, body);
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

	/** A note or a page asks to move; the dialog asks where. */
	askMove = (note: Note) => {
		this.moving = note;
	};

	/**
	 * Move a note or a page to another space. It leaves the views here, the
	 * editor, the dock and its own page included. Resolves to an error for
	 * the dialog to show, or null once moved.
	 */
	moveTo = async (note: Note, space: string): Promise<string | null> => {
		try {
			if (isPage(note)) await movePage(note.date, note.id, space);
			else await moveNote(note.date, note.id, space);
			if (this.editing?.id === note.id) this.editing = null;
			if (this.docked?.id === note.id) this.docked = null;
			if (page.params.id === note.id) await this.openDay(this.day);
			await this.refresh();
			return null;
		} catch (e) {
			return String(e);
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
	 * a page from the last one would mean nothing there. Its days are listed
	 * afresh.
	 */
	switchSpace = () => {
		this.switching = this.leaveSpace();
		return this.switching;
	};

	/** The views catching up with the space opened last. */
	private switching: Promise<void> = Promise.resolve();

	private leaveSpace = async () => {
		this.query = '';
		this.docked = null;
		const route = page.route.id;
		if (
			route === '/(app)/search' ||
			route === '/(app)/similar/[date]/[id]' ||
			route === '/(app)/thread/[id]' ||
			route === '/(app)/page/[date]/[id]'
		)
			await this.openDay(this.day);
		await Promise.all([this.refresh(), this.loadThreads()]);
	};

	/** What every card can do. */
	cardActions = $derived({
		onedit: (note: Note) => (this.editing = note),
		ondelete: (note: Note) => (this.deleting = note),
		onsave: this.saveBody,
		onsimilar: this.canSimilar ? this.showSimilar : undefined,
		onopen: (note: Note) => void this.openPage(note),
		ondock: this.dock,
		onpage: (note: Note) => (this.turning = note),
		onmove: this.canMove ? this.askMove : undefined,
		onerror: this.showError
	});
}

export const [getShell, setShell] = createContext<Shell>();
