import { toast } from 'svelte-sonner';
import { invalidate } from '$app/navigation';
import { page } from '$app/state';
import {
	dismissThread,
	getNotes,
	keepOutOfThreads,
	keepThread,
	listThreads,
	mergeThreads,
	patchSettings,
	putInThread,
	undoThreadChange,
	type Note,
	type Thread,
	type ThreadOrder
} from '#lib/api.js';
import { noteTitle } from '#lib/markdown.js';
import { m } from '#lib/paraglide/messages.js';
import type { Shell } from '#lib/shell.svelte.js';
import { threadName, threadScope } from '#lib/notes/threads.js';

/** Where a note sits in its thread: the thread, and its place in it from 0. */
interface ThreadPlace {
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

/** What the shell keeps to itself and lends the threads, to say how a change went. */
export interface ThreadErrors {
	/** The operation that failed with `what` worked since. */
	cleared: (what: string) => void;
	/** A failure away from what the user is doing, with another try. */
	failQuietly: (what: string, e: unknown, retry?: () => unknown) => void;
}

/**
 * The threads of the open space as the views show them (SPEC 6.4): where
 * each note sits, what a thread is called, and what the user does to them,
 * each change with an Undo. The shell holds one, as `shell.threads`.
 */
export class ThreadsState {
	#shell: Shell;
	#errors: ThreadErrors;

	/** The threads of the user's each note of the open space is in, by
	 *  note: one at most of the general scope, and one at most of each name
	 *  it mentions, the general scope's first. A note in a thread only
	 *  suggested has none. */
	byNote = $state.raw<Record<string, ThreadPlace[]>>({});
	/** Every thread of the open space, kept or suggested. */
	threadList = $state.raw<Thread[]>([]);
	/** How many threads are only suggested. */
	suggested = $state(0);
	/** The notes taken out of threads. */
	alone = $state.raw<string[]>([]);
	/** The thread order setting: which of a thread's notes its view lists first. */
	threadOrder = $state<ThreadOrder>('oldest');
	/** Notes waiting on the thread picker. */
	picking = $state<ThreadPick | null>(null);
	/** The first note of each untitled thread, by its id, named as a line
	 *  names it: what `nameOf` calls those threads. */
	leads = $state.raw<Record<string, string>>({});
	/** First notes asked for and not read yet, so each is asked for once. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- nothing is drawn from it
	#leadsAsked = new Set<string>();
	/** Threads' first notes to read again with the next reload: those changed,
	 *  or all of them when what changed is not known. */
	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- nothing is drawn from it
	#leadsChanged: Set<string> | 'all' = new Set();

	constructor(shell: Shell, errors: ThreadErrors) {
		this.#shell = shell;
		this.#errors = errors;
	}

	/** Where a note sits in its first thread, while threads are on offer. */
	threadOf = (id: string): ThreadPlace | undefined => this.threadsOf(id)[0];

	/** Where a note sits in each of its threads, while threads are on offer. */
	threadsOf = (id: string): ThreadPlace[] =>
		this.#shell.canSimilar ? (this.byNote[id] ?? []) : [];

	/** A note taken out of threads, which can be let back in while threads are on offer. */
	keptOut = (id: string) => this.#shell.canSimilar && this.alone.includes(id);

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

	/** Note `changed` changed, or every note when it is not known, as a
	 *  thread's first note may have. */
	leadChanged = (changed?: string) => {
		if (changed === undefined) this.#leadsChanged = 'all';
		else if (this.#leadsChanged !== 'all') this.#leadsChanged.add(changed);
	};

	/** The first notes changed since it was last asked, for `readLeadsAgain`. */
	takeChangedLeads = () => {
		const taken = this.#leadsChanged;
		// eslint-disable-next-line svelte/prefer-svelte-reactivity -- nothing is drawn from it
		this.#leadsChanged = new Set();
		return taken;
	};

	/** Read again the first notes taken, once the notes reloaded: their
	 *  names change, the old ones showing meanwhile. Only those changed, as
	 *  a space can name hundreds of threads by their first notes. */
	readLeadsAgain = (taken: Set<string> | 'all') => {
		this.#readLeads(
			taken === 'all' ? Object.keys(this.leads) : [...taken].filter((id) => id in this.leads),
			true
		);
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

			this.byNote = places;
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
			this.#shell.reloads++;
		} catch (e) {
			this.#errors.failQuietly(m.error_load_threads(), e, this.loadThreads);
		}
	};

	/** Take a note out of threads, where it stays, or let it back in. */
	keepOut = async (note: Pick<Note, 'id'>, out: boolean) => {
		try {
			await keepOutOfThreads(note.id, out);
			this.#errors.cleared(m.error_thread_change());
			if (out) this.#offerUndo(m.thread_taken_out());
		} catch (e) {
			this.#shell.fail(m.error_thread_change(), e, () => this.keepOut(note, out));
		}
	};

	/** Make a suggested thread the user's, or stop suggesting it. */
	keepThread = async (id: string, keep: boolean) => {
		const shell = this.#shell;
		try {
			await (keep ? keepThread(id) : dismissThread(id));
			this.#errors.cleared(m.error_keep_thread());
			// Kept, the thread is the user's: said so, as it changes what happens next.
			if (keep) return this.#offerUndo(m.thread_kept());
			// Dismissed from its own page or the dock, the thread is gone from
			// under the user: they go on to the threads, and Undo brings it back.
			const here = page.params.id === id;
			const docked = shell.dockedThread === id;
			if (docked) shell.dockedThread = null;
			if (here) await shell.showThreads();
			this.#offerUndo(m.thread_dismissed(), async () => {
				if (docked) shell.dockedThread = id;
				if (here) await shell.showThread(id);
			});
		} catch (e) {
			shell.fail(m.error_keep_thread(), e, () => this.keepThread(id, keep));
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
			if (!(await undoThreadChange())) return this.#shell.showError(m.thread_undo_late());
			await back?.();
		} catch (e) {
			this.#shell.fail(m.error_keep_thread(), e);
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
		const shell = this.#shell;
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
			if (pick.kind === 'merge' && thread && shell.dockedThread === pick.from)
				shell.dockedThread = thread;
			if (pick.kind === 'merge' && thread && page.params.id === pick.from)
				await shell.showThread(thread);
			if (pick.kind === 'merge' && pick.from && thread) {
				const [from, into] = [pick.from, thread];
				// Undone, the thread merged away comes back where the other took its place.
				this.#offerUndo(m.thread_merged({ from: fromName, into: intoName }), async () => {
					if (shell.dockedThread === into) shell.dockedThread = from;
					if (page.params.id === into) await shell.showThread(from);
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

	/** Change the order of a thread's notes, the setting Settings > Threads
	 *  has, from the thread itself. */
	setThreadOrder = async (order: ThreadOrder) => {
		const was = this.threadOrder;
		this.threadOrder = order;
		try {
			await patchSettings({ threadOrder: order });
		} catch (e) {
			this.threadOrder = was;
			this.#shell.fail(m.error_save_setting(), e);
		}
	};
}
