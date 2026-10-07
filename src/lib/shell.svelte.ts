import { createContext, tick, type Snippet } from 'svelte';
import { toast } from 'svelte-sonner';
import { EditorView } from '@codemirror/view';
import { goto, invalidate } from '$app/navigation';
import { resolve } from '$app/paths';
import { navigating, page } from '$app/state';
import {
	clearDayAhead,
	deleteNote,
	deletePage,
	dismissThread,
	getNotes,
	isPage,
	keepOutOfThreads,
	keepThread,
	listPins,
	listThreads,
	mergeThreads,
	moveNote,
	movePage,
	noteToPage,
	putInThread,
	setPins,
	undoThreadChange,
	updateNote,
	type Note,
	type Pin,
	type SpacesView,
	type Thread,
	type ThreadOrder
} from '#lib/api.js';
import { loadDock, saveDock, type DockSide } from '#lib/dock.js';
import { addToPageDraft } from '#lib/page-draft.js';
import { m } from '#lib/paraglide/messages.js';
import { samePin } from '#lib/pins.js';
import { noteTitle } from '#lib/markdown.js';
import { threadHref, threadName, threadScope } from '#lib/threads.js';
import { notesChanged, type WorkspaceHost } from '#lib/plugins/app.js';

/** Where a note sits in its thread: the thread, and its place in it from 0. */
export interface ThreadPlace {
	thread: Thread;
	index: number;
}

/** Notes waiting on the thread picker to say where they go (SPEC 6.4). */
export interface ThreadPick {
	/** One note added to a thread, notes moved from one, or a whole thread merged. */
	kind: 'add' | 'move' | 'merge';
	notes: string[];
	/** The thread they come from, which is not on offer. */
	from?: string;
}

/** A view's title row, which the top bar shows a copy of once it scrolls away. */
export interface ViewTitle {
	/** Where the back arrow leads when no view is behind this one in the
	 *  history. Left out on a day, which has its own arrows. */
	back?: string;
	title?: string;
	/** What the title row's pin button pins to the left edge (SPEC 3.13). */
	pin?: Pin;
	/** After the title, dimmed: a search's whole query. */
	detail?: string;
	/** A title not the user's yet, as a suggested thread's: set lighter. */
	tentative?: boolean;
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
	/** What went wrong, in the app's own words, over the views. */
	error = $state<string | null>(null);
	/** The error as it came, under Details, for a report; null for a message alone. */
	errorDetail = $state<string | null>(null);
	/** Does what failed again, for a failure worth another try. */
	errorRetry = $state<(() => unknown) | null>(null);
	/** Similar notes come from the embedding model's vectors. */
	embeddingInstalled = $state(false);
	canSimilar = $derived(this.embeddingInstalled);
	/** The threads of the user's each note of the open space is in, by
	 *  note (SPEC 6.4): one at most of the general scope, and one at most
	 *  of each name it mentions, the general scope's first. A note in a
	 *  thread only suggested has none. */
	threads = $state.raw<Record<string, ThreadPlace[]>>({});
	/** Every thread of the open space, kept or suggested. */
	threadList = $state.raw<Thread[]>([]);
	/** How many threads are only suggested. */
	suggested = $state(0);
	/** What the open space has pinned to the left edge, in its order. */
	pins = $state.raw<Pin[]>([]);
	/** The notes taken out of threads. */
	alone = $state.raw<string[]>([]);
	/** The text size setting, in pixels: the rem the day's columns are sized in. */
	textSize = $state(16);
	/** The thread order setting: which of a thread's notes its view lists first. */
	threadOrder = $state<ThreadOrder>('oldest');
	/** The capture hotkey setting, as an accelerator; null until it is read. */
	captureHotkey = $state<string | null>(null);
	/** The views' width, which a docked page narrows. Taken with the
	 *  scrollbar, which comes and goes with what the columns hold. */
	width = $state(0);

	/** The note waiting on the delete confirmation. */
	deleting = $state<Note | null>(null);
	/** The note waiting on a title to become a page. */
	turning = $state<Note | null>(null);
	/** The note or page waiting on the space it moves to. */
	moving = $state<Note | null>(null);
	/** Notes waiting on the thread picker. */
	picking = $state<ThreadPick | null>(null);
	/** Every space and the open one, as the app layout last listed them. */
	spaces = $state.raw<SpacesView | null>(null);
	/** A note can move only with another space to go to. */
	canMove = $derived((this.spaces?.spaces.length ?? 0) > 1);
	/** The page docked beside the view, to write in while the day stays in reach. */
	docked = $state<Note | null>(null);
	/** A plugin's view docked beside the view instead, by its type. */
	panel = $state<string | null>(null);
	/** Or a thread, by its id. */
	dockedThread = $state<string | null>(null);
	dockOpen = $derived(this.docked !== null || this.panel !== null || this.dockedThread !== null);
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
	/** Counts the times the notes or threads were read again, for a view
	 *  outside the routes, which loads what it shows itself. */
	reloads = $state(0);
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

	/** The history entry the views opened on: going back past it would leave
	 *  them, for the onboarding or nothing. */
	#firstEntry: number | undefined;

	/** Note where the history stands, the first time the views are shown. */
	markFirstEntry = () => {
		if ('navigation' in window) this.#firstEntry ??= window.navigation.currentEntry?.index;
	};

	/** The back arrow returns to the view it was opened from. With none
	 *  behind it, or no way to tell, it follows its link. */
	goBack = (event: MouseEvent) => {
		const index = 'navigation' in window ? window.navigation.currentEntry?.index : undefined;
		if (index === undefined || this.#firstEntry === undefined || index <= this.#firstEntry) return;
		event.preventDefault();
		history.back();
	};

	/** Load what the views show again, after a change here or on disk. */
	refresh = async () => {
		// An invalidation aborts a navigation under way, so it waits for one to land.
		while (navigating.complete) await navigating.complete.catch(() => {});
		await invalidate('app:notes');
		this.reloads++;
		// A note edited may be a thread's first: its name is read again, the
		// old one showing meanwhile.
		this.#readLeads(Object.keys(this.leads), true);
		// The plugins' pages and panels load their notes themselves.
		notesChanged();
	};

	showError = (message: string) => {
		this.error = message;
		this.errorDetail = null;
		this.errorRetry = null;
	};

	/** Say what failed in plain words, keeping the error itself under Details. */
	fail = (what: string, e: unknown, retry?: () => unknown) => {
		this.error = what;
		this.errorDetail = e instanceof Error ? e.message : String(e);
		this.errorRetry = retry ?? null;
	};

	/** The operation that failed with `what` worked since: its error goes,
	 *  and only its own, so a success elsewhere never hides a failure. */
	#cleared = (what: string) => {
		if (this.error === what) this.error = null;
	};

	/** A failure away from what the user is doing, such as reading the pins:
	 *  a toast with another try, rather than a banner over the page. */
	#failQuietly = (what: string, e: unknown, retry?: () => unknown) => {
		console.error(e);
		toast.error(what, {
			action: retry ? { label: m.error_retry(), onClick: () => void retry() } : undefined
		});
	};

	/** The saves that failed, by note, with the error as it came: each note
	 *  shows its own under its text or its editor, until it saves. */
	saveFailures = $state.raw<Record<string, string>>({});

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

	/** Where a note sits in its first thread, while threads are on offer. */
	threadOf = (id: string): ThreadPlace | undefined => this.threadsOf(id)[0];

	/** Where a note sits in each of its threads, while threads are on offer. */
	threadsOf = (id: string): ThreadPlace[] => (this.canSimilar ? (this.threads[id] ?? []) : []);

	/** A note taken out of threads, which can be let back in while threads are on offer. */
	keptOut = (id: string) => this.canSimilar && this.alone.includes(id);

	/** The first note of each untitled thread, by its id, named as a line
	 *  names it: what `nameOf` calls those threads. */
	leads = $state.raw<Record<string, string>>({});
	/** First notes asked for and not read yet, so each is asked for once. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- nothing is drawn from it
	#leadsAsked = new Set<string>();

	/**
	 * A thread's name, as every view gives it (`threadName`). An untitled
	 * one is called by its first note, read here the first time it is
	 * named; until then it is "Thread".
	 */
	nameOf = (thread: Thread) => {
		if (thread.title) return thread.title;
		const first = thread.notes[0];
		if (first === undefined) return threadName(thread);
		if (!(first in this.leads)) this.#readLeads([first]);
		return this.leads[first] || threadName(thread);
	};

	/** Read these first notes, those not read or asked for yet, together;
	 *  `again` reads those already read too. */
	#readLeads = (ids: (string | undefined)[], again = false) => {
		const fresh = ids.filter(
			(id): id is string =>
				id !== undefined && (again || !(id in this.leads)) && !this.#leadsAsked.has(id)
		);
		if (!fresh.length) return;
		fresh.forEach((id) => this.#leadsAsked.add(id));
		// Asked for from a view as it draws: the answer lands after it.
		queueMicrotask(async () => {
			try {
				const notes = await getNotes(fresh);
				this.leads = {
					...this.leads,
					...Object.fromEntries(notes.map((note) => [note.id, noteTitle(note)]))
				};
			} catch {
				// A name not read stays "Thread"; nothing else depends on it.
			} finally {
				fresh.forEach((id) => this.#leadsAsked.delete(id));
			}
		});
	};

	/** Read the open space's threads again, as the backend placed them. */
	loadThreads = async () => {
		try {
			const view = await listThreads();
			const places: Record<string, ThreadPlace[]> = {};

			for (const thread of view.threads)
				if (thread.kept)
					thread.notes.forEach((id, index) => (places[id] ??= []).push({ thread, index }));
			for (const list of Object.values(places))
				list.sort((a, b) => Number(a.thread.scope !== null) - Number(b.thread.scope !== null));

			this.threads = places;
			this.threadList = view.threads;
			// The threads the day names under their notes, read before they show.
			this.#readLeads(
				view.threads
					.filter((thread) => thread.kept && !thread.title)
					.map((thread) => thread.notes[0])
			);
			this.suggested = view.threads.filter((thread) => !thread.kept).length;
			this.alone = view.alone;
			// The view of a thread lists its notes itself.
			await invalidate('app:threads');
			this.reloads++;
		} catch (e) {
			this.#failQuietly(m.error_load_threads(), e, this.loadThreads);
		}
	};

	showThread = (id: string) => goto(threadHref(id));

	/** Read the open space's pins again. */
	loadPins = async () => {
		try {
			this.pins = await listPins();
		} catch (e) {
			this.#failQuietly(m.error_load_pins(), e, this.loadPins);
		}
	};

	isPinned = (pin: Pin) => this.pins.some((other) => samePin(other, pin));

	/** Save the pins in this order. */
	#savePins = async (pins: Pin[]) => {
		try {
			this.pins = await setPins(pins);
		} catch (e) {
			this.#failQuietly(m.error_save_pins(), e, () => this.#savePins(pins));
		}
	};

	/** Pin a view to the left edge, last, or unpin it. A thread only
	 *  suggested is kept as it is pinned: one worth a click is the user's. */
	togglePin = async (pin: Pin) => {
		if (this.isPinned(pin))
			return this.#savePins(this.pins.filter((other) => !samePin(other, pin)));
		const thread = pin.kind === 'thread' && this.threadList.find((t) => t.id === pin.target);
		if (thread && !thread.kept) await this.keepThread(thread.id, true);
		await this.#savePins([...this.pins, pin]);
	};

	/** Move a pin one place up or down the edge, past the pin `shown` has
	 *  next to it: the edge leaves out those that lead nowhere for now. */
	movePin = (pin: Pin, by: -1 | 1, shown: Pin[]) => {
		const next = shown[shown.findIndex((other) => samePin(other, pin)) + by];
		const from = this.pins.findIndex((other) => samePin(other, pin));
		const to = next ? this.pins.findIndex((other) => samePin(other, next)) : -1;
		if (from < 0 || to < 0) return;
		const pins = [...this.pins];
		[pins[from], pins[to]] = [pins[to], pins[from]];
		return this.#savePins(pins);
	};

	/** Forget the day ahead read in a note, for one read wrong. */
	clearDayAhead = async (note: Pick<Note, 'id' | 'date'>) => {
		try {
			await clearDayAhead(note.date, note.id);
			this.#cleared(m.error_clear_day_ahead());
			await this.refresh();
		} catch (e) {
			this.fail(m.error_clear_day_ahead(), e, () => this.clearDayAhead(note));
		}
	};

	/** Take a note out of threads, where it stays, or let it back in. */
	keepOut = async (note: Pick<Note, 'id'>, out: boolean) => {
		try {
			await keepOutOfThreads(note.id, out);
			this.#cleared(m.error_thread_change());
			if (out) this.#offerUndo(m.thread_taken_out());
		} catch (e) {
			this.fail(m.error_thread_change(), e, () => this.keepOut(note, out));
		}
	};

	/** Make a suggested thread the user's, or stop suggesting it. */
	keepThread = async (id: string, keep: boolean) => {
		try {
			await (keep ? keepThread(id) : dismissThread(id));
			this.#cleared(m.error_keep_thread());
			// Kept, the thread is the user's: said so, as it changes what happens next.
			if (keep) return this.#offerUndo(m.thread_kept());
			// Dismissed from its own page or the dock, the thread is gone from
			// under the user: they go on to the threads, and Undo brings it back.
			const here = page.params.id === id;
			const docked = this.dockedThread === id;
			if (docked) this.dockedThread = null;
			if (here) await this.showThreads();
			this.#offerUndo(m.thread_dismissed(), async () => {
				if (docked) this.dockedThread = id;
				if (here) await this.showThread(id);
			});
		} catch (e) {
			this.fail(m.error_keep_thread(), e, () => this.keepThread(id, keep));
		}
	};

	/**
	 * Offer to take back the change to threads just made, with an Undo on a
	 * toast. `back` runs once it is taken back, to show what came back.
	 */
	#offerUndo = (message: string, back?: () => unknown) => {
		toast(message, {
			duration: 8000,
			action: { label: m.thread_undo(), onClick: () => void this.#undo(back) }
		});
	};

	#undo = async (back?: () => unknown) => {
		try {
			// Refused once anything changed the threads since: a later change is never lost.
			if (!(await undoThreadChange())) return this.showError(m.thread_undo_late());
			await back?.();
		} catch (e) {
			this.fail(m.error_keep_thread(), e);
		}
	};

	/** A note asks for a thread; the picker asks which. */
	askThread = (note: Pick<Note, 'id'>) => {
		const from = this.threadOf(note.id)?.thread.id;
		this.picking = { kind: from ? 'move' : 'add', notes: [note.id], from };
	};

	/**
	 * Put the picked notes in thread `into`, or a new one when null, and
	 * open it when they came from the thread on view. Resolves to an error
	 * for the picker to show, or null once done.
	 */
	pickThread = async (pick: ThreadPick, into: string | null): Promise<string | null> => {
		// Named before the change, which can take a thread away.
		const name = (id: string | null | undefined) => {
			const thread = id ? this.threadList.find((other) => other.id === id) : undefined;
			return thread ? this.nameOf(thread) : m.thread_untitled();
		};
		const [fromName, intoName] = [name(pick.from), name(into)];
		const count = pick.notes.length;
		try {
			let thread = into;
			if (pick.kind === 'merge' && pick.from && into) await mergeThreads(pick.from, into);
			// A new thread is made among the notes of the name they come from.
			else thread = await putInThread(pick.notes, into, pick.from && threadScope(pick.from));
			if (pick.kind === 'merge' && thread && this.dockedThread === pick.from)
				this.dockedThread = thread;
			if (pick.kind === 'merge' && thread && page.params.id === pick.from)
				await this.showThread(thread);
			if (pick.kind === 'merge' && pick.from && thread) {
				const [from, into] = [pick.from, thread];
				// Undone, the thread merged away comes back where the other took its place.
				this.#offerUndo(m.thread_merged({ from: fromName, into: intoName }), async () => {
					if (this.dockedThread === into) this.dockedThread = from;
					if (page.params.id === into) await this.showThread(from);
				});
			} else {
				this.#offerUndo(
					into === null
						? m.thread_moved_new({ count })
						: pick.kind === 'add'
							? m.thread_added({ into: intoName })
							: m.thread_moved({ count, into: intoName })
				);
			}
			return null;
		} catch (e) {
			return String(e);
		}
	};

	showThreads = () => goto(resolve('threads/'));

	showPages = () => goto(resolve('pages/'));

	showCalendar = () => goto(resolve('calendar/'));

	showMentions = () => goto(resolve('mentions/'));

	/** A plugin's page, `/plugin/<type>/`, with its query string. */
	openPluginPage = (type: string, params: Record<string, string> = {}) => {
		const query = Object.entries(params)
			.map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(value)}`)
			.join('&');
		// resolve() takes no query string, so the query follows the path it gives.
		// eslint-disable-next-line svelte/no-navigation-without-resolve
		return goto(`${resolve(`plugin/${type}/`)}${query ? `?${query}` : ''}`);
	};

	/** The dock holds one thing: a page, a plugin's view or a thread. */
	dock = (note: Note) => {
		this.panel = null;
		this.dockedThread = null;
		this.docked = note;
	};

	openPanel = (type: string) => {
		this.docked = null;
		this.dockedThread = null;
		this.panel = type;
	};

	/** Dock a thread, brought to `note` when it was opened from that note's line. */
	dockThread = (id: string, note?: string) => {
		this.docked = null;
		this.panel = null;
		this.dockedThread = id;
		this.dockNote = note ?? null;
	};

	/** The note the docked thread opens on, until it is in sight. */
	dockNote = $state<string | null>(null);

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
		await this.blink(entry.id);
	};

	/** Bring a note on view into sight and blink it, the first found in
	 *  `within`, as the thread's lane does inside its own view. With `focus`,
	 *  the keyboard goes on from the note rather than from what was pressed. */
	blink = async (id: string, within: ParentNode = document, { focus = false } = {}) => {
		// Cleared first so a second click on the same note blinks it again.
		this.blinking = null;
		await tick();
		this.blinking = id;
		await tick();
		const note = within.querySelector<HTMLElement>(`[data-note-id="${CSS.escape(id)}"]`);
		const still = matchMedia('(prefers-reduced-motion: reduce)').matches;
		note?.scrollIntoView({ block: 'center', behavior: still ? 'instant' : 'smooth' });
		if (note && focus) {
			// Reachable by script only, so Tab still skips the note itself.
			if (!note.hasAttribute('tabindex')) note.tabIndex = -1;
			note.focus({ preventScroll: true });
		}
		clearTimeout(this.#blinkTimer);
		// A little past the animation, which runs 1.2s.
		this.#blinkTimer = setTimeout(() => (this.blinking = null), 1400);
	};

	/** The inline editor. */
	saveBody = async (note: Note, body: string): Promise<boolean> => {
		try {
			if (body.trim() !== note.body) await updateNote(note.date, note.id, body);
			if (note.id in this.saveFailures)
				this.saveFailures = Object.fromEntries(
					Object.entries(this.saveFailures).filter(([id]) => id !== note.id)
				);
			await this.refresh();
			return true;
		} catch (e) {
			// Shown on the note itself: the editor keeps the text, and its
			// Save is the way to try again.
			this.saveFailures = {
				...this.saveFailures,
				[note.id]: e instanceof Error ? e.message : String(e)
			};
			return false;
		}
	};

	/** Resolves to an error for the dialog to show, or null once done. */
	turnIntoPage = async (note: Note, title: string): Promise<string | null> => {
		try {
			const created = await noteToPage(note.date, note.id, title);
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
	 * dock and its own page included. Resolves to an error for
	 * the dialog to show, or null once moved.
	 */
	moveTo = async (note: Note, space: string): Promise<string | null> => {
		try {
			if (isPage(note)) await movePage(note.date, note.id, space);
			else await moveNote(note.date, note.id, space);
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
			if (this.docked?.id === note.id) this.docked = null;
			// The page itself, or the notes similar to it, go back to the day.
			if (page.params.id === note.id) await this.openDay(this.day);
			this.#cleared(m.error_delete_note());
			await this.refresh();
		} catch (e) {
			this.fail(m.error_delete_note(), e, () => this.remove(note));
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
		this.dockedThread = null;
		const route = page.route.id;
		if (
			route === '/(app)/search' ||
			route === '/(app)/similar/[date]/[id]' ||
			route === '/(app)/thread/[id]' ||
			route === '/(app)/mention/[name]' ||
			route === '/(app)/page/[date]/[id]'
		)
			await this.openDay(this.day);
		await Promise.all([this.refresh(), this.loadThreads(), this.loadPins()]);
	};

	/** What every card can do. */
	cardActions = $derived({
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
