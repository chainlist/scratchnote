import { createContext, tick, type Snippet } from 'svelte';
import { toast } from 'svelte-sonner';
import { EditorView } from '@codemirror/view';
import { goto, invalidate } from '$app/navigation';
import { resolve } from '$app/paths';
import { page } from '$app/state';
import {
	clearDayAhead,
	deleteNote,
	deletePage,
	isPage,
	listPins,
	moveNote,
	movePage,
	noteToPage,
	setPins,
	updateNote,
	type Note,
	type Pin,
	type SpacesView
} from '#lib/api.js';
import { loadDock, saveDock, type DockSide } from '#lib/shell/dock.js';
import { afterNavigation, onGuard } from '#lib/app/back.svelte.js';
import { addToPageDraft } from '#lib/shell/page-draft.js';
import { m } from '#lib/paraglide/messages.js';
import { errorText } from '#lib/helpers/errors.js';
import { pluginPageHref, samePin } from '#lib/shell/pins.js';
import { threadHref } from '#lib/notes/threads.js';
import { notesChanged, type WorkspaceHost } from '#lib/plugins/app.js';
import { ThreadsState } from '#lib/shell/threads.svelte.js';

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
	/** What the window's title calls the view, where `heading` stands in for `title`. */
	name?: string;
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
	/** The threads of the open space, and what the user does to them. */
	readonly threads = new ThreadsState(this, {
		cleared: (what) => this.#cleared(what),
		failQuietly: (what, e, retry) => this.#failQuietly(what, e, retry)
	});
	/** What the open space has pinned to the left edge, in its order. */
	pins = $state.raw<Pin[]>([]);
	/** The rem in pixels, the text size setting times Android's font scale: what
	 *  the day's columns are sized in. */
	textSize = $state(16);
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
	/** The space switcher's list, open from the command center too. */
	spacesOpen = $state(false);
	/** On a phone, the ribbon drawn out from the left edge. */
	ribbonOpen = $state(false);
	/** The views' scrolling element, which takes the day's swipes. */
	main = $state<HTMLElement | null>(null);
	/** The view opening slid into place under the finger already, so it does
	 *  not rise as an opening view does. */
	slid = false;
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
		// Past the entry Android's Back keeps while something is open, such as the dock.
		const kept = onGuard() ? 1 : 0;
		if (index === undefined || this.#firstEntry === undefined || index - kept <= this.#firstEntry)
			return;
		event.preventDefault();
		history.go(-1 - kept);
	};

	/** Load what the views show again, after a change here or on disk;
	 *  `changed` is the note that changed, when only one did. */
	refresh = async (changed?: string) => {
		this.threads.leadChanged(changed);
		// An invalidation aborts a navigation under way, so it waits for one to land.
		await afterNavigation();
		const leads = this.threads.takeChangedLeads();
		await invalidate('app:notes');
		this.reloads++;
		// A note edited may be a thread's first: its name is read again.
		this.threads.readLeadsAgain(leads);
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
		this.errorDetail = errorText(e);
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
		await this.#switching.catch(() => {});
		if (this.draft) return this.draft(body);
		addToPageDraft(body);
		this.newPage();
	};

	showSimilar = (note: Note) => {
		void goto(resolve(`similar/${note.date}/${note.id}/`));
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

	/** The pins as the user last saved them: the left edge animates that
	 *  change only, not pins read at launch, on a space switch or as the
	 *  threads they lead to come in. Nothing is drawn from it. */
	pinsSaved: Pin[] | null = null;

	/** Save the pins in this order. */
	#savePins = async (pins: Pin[]) => {
		try {
			this.pins = this.pinsSaved = await setPins(pins);
		} catch (e) {
			this.#failQuietly(m.error_save_pins(), e, () => this.#savePins(pins));
		}
	};

	/** Pin a view to the left edge, last, or unpin it. A thread only
	 *  suggested is kept as it is pinned: one worth a click is the user's. */
	togglePin = async (pin: Pin) => {
		if (this.isPinned(pin))
			return this.#savePins(this.pins.filter((other) => !samePin(other, pin)));
		const thread =
			pin.kind === 'thread' && this.threads.threadList.find((t) => t.id === pin.target);
		if (thread && !thread.kept) await this.threads.keepThread(thread.id, true);
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
			await this.refresh(note.id);
		} catch (e) {
			this.fail(m.error_clear_day_ahead(), e, () => this.clearDayAhead(note));
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
		return goto(pluginPageHref(query ? `${type}?${query}` : type));
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
			await this.refresh(note.id);
			return true;
		} catch (e) {
			// Shown on the note itself: the editor keeps the text, and its
			// Save is the way to try again.
			this.saveFailures = {
				...this.saveFailures,
				[note.id]: errorText(e)
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
		this.#switching = this.#leaveSpace();
		return this.#switching;
	};

	/** The views catching up with the space opened last. */
	#switching: Promise<void> = Promise.resolve();

	#leaveSpace = async () => {
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
		await Promise.all([this.refresh(), this.threads.loadThreads(), this.loadPins()]);
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
