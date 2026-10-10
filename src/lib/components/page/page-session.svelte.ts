import {
	createPage,
	finishPage,
	getPage,
	onIndexRebuilt,
	onNoteUpdated,
	renamePage,
	stopAll,
	updatePage,
	type Note
} from '#lib/api.js';
import { wordCount } from '#lib/markdown.js';
import { joinText, pageDraft } from '#lib/page-draft.js';
import { m } from '#lib/paraglide/messages.js';

/** How long typing has to stop before the text saves itself. */
const QUIET_MS = 1000;

const cleanTitle = (raw: string) => raw.split(/\s+/).filter(Boolean).join(' ');

/** What the page view needs from where it is mounted. */
interface PageHost {
	/** The day a new page goes on. */
	readonly date: string;
	/** A new page got its title and was saved. */
	oncreated: (page: Note) => void;
	/** The title is being typed in, so a reload leaves it be. */
	editingTitle: () => boolean;
}

/**
 * One page open in a view: its title and text as typed, saved by
 * themselves once typing stops, and taken in again when they change on
 * disk. Made as the view starts, as its autosave is an effect.
 */
export class PageSession {
	/** The page as last saved or read, null until a new one has a title. */
	page = $state<Note | null>(null);
	title = $state('');
	body = $state('');
	/** The text the file holds, to tell typing apart from what was loaded. */
	savedBody = $state('');
	/** The text the view opened with: recall waits for it to change. */
	openedWith = $state('');
	loading = $state(true);
	/** What failed in plain words, and the error as it came for its tooltip. */
	error = $state<{ what: string; detail?: string } | null>(null);
	saving = $state(false);
	/** Something was saved since the view opened, which the status then says. */
	saved = $state(false);

	words = $derived(wordCount(this.body));
	/** The status line: what failed, what is going on, or that all is saved. */
	status = $derived(
		this.error
			? this.error.what
			: this.saving
				? m.common_saving()
				: !this.page && this.body.trim()
					? m.pages_needs_title()
					: this.saved && this.body === this.savedBody
						? m.pages_saved()
						: ''
	);

	#host: PageHost;
	#closed = false;
	#timer: ReturnType<typeof setTimeout> | undefined;
	/** Every write waits for the one before, so a new page is created once
	 *  and each save sends the text as it is by then. */
	#chain: Promise<void> = Promise.resolve();

	constructor(host: PageHost) {
		this.#host = host;
		// The editor binds `body`, so a change to it is typing, or a reload.
		$effect(() => {
			if (this.loading || this.body === this.savedBody) return;
			clearTimeout(this.#timer);
			this.#timer = setTimeout(() => {
				this.#timer = undefined;
				void this.#queue(this.#saveBody);
			}, QUIET_MS);
		});
	}

	#queue(write: () => Promise<void>) {
		this.#chain = this.#chain.then(write);
		return this.#chain;
	}

	/** Read the page, or the draft of a new one when `id` is null. */
	async open(id: string | null) {
		if (id === null) {
			this.title = pageDraft.title;
			this.body = pageDraft.body;
			this.savedBody = this.body;
			this.openedWith = this.body;
			this.loading = false;
			return;
		}
		try {
			this.#load(await getPage(id));
			this.openedWith = this.body;
		} catch (e) {
			this.error = { what: m.error_load_page(), detail: String(e) };
		}
		this.loading = false;
	}

	/** Follow edits in another editor; returns the stop. */
	listen() {
		return stopAll(
			onNoteUpdated((changed) => void this.#refresh(changed)),
			onIndexRebuilt(() => void this.#refresh())
		);
	}

	/** The view is left: the last save, then the embedding model gets the page. */
	close() {
		this.#closed = true;
		clearTimeout(this.#timer);
		// A page still being created is finished by `saveTitle`.
		if (!this.page) {
			this.#keepDraft();
			return;
		}
		// The last save first, then the embedding model gets the page, once (SPEC 3.5).
		void this.#queue(async () => {
			await this.#saveBody();
			if (this.page) await finishPage(this.page.id).catch(() => {});
		});
	}

	#load(fresh: Note) {
		this.page = fresh;
		// Not while it is being typed in.
		if (!this.#host.editingTitle()) this.title = fresh.subject ?? '';
		this.body = fresh.body;
		this.savedBody = fresh.body;
	}

	/** Take in what changed on disk, unless it would overwrite unsaved typing. */
	async #refresh(changed?: string) {
		const current = this.page;
		if (!current || (changed !== undefined && changed !== current.id)) return;
		try {
			const fresh = await getPage(current.id);
			if (this.body !== this.savedBody || this.#timer !== undefined) {
				this.page = { ...fresh, subject: current.subject, body: current.body };
			} else {
				this.#load(fresh);
			}
		} catch {
			// Gone, say deleted in another editor; the day view will say so.
		}
	}

	#keepDraft() {
		pageDraft.title = this.title;
		pageDraft.body = this.body;
	}

	/** Text handed over from the capture window, after what is typed. */
	addText(text: string) {
		this.body = joinText(this.body, text);
	}

	/** Save the text now rather than once typing stops. */
	saveNow() {
		clearTimeout(this.#timer);
		this.#timer = undefined;
		return this.#queue(this.#saveBody);
	}

	#saveBody = async () => {
		if (!this.page) {
			this.#keepDraft();
			return;
		}
		const text = this.body;
		if (text.trim() === this.savedBody.trim()) return;
		this.saving = true;
		try {
			const fresh = await updatePage(this.page.id, text);
			this.savedBody = text;
			this.page = { ...fresh, subject: this.page.subject };
			this.saved = true;
			this.error = null;
		} catch (e) {
			this.error = { what: m.error_save_page(), detail: String(e) };
		} finally {
			this.saving = false;
		}
	};

	/** Enter or leaving the field: a new page is created, an existing one retitled. */
	saveTitle() {
		return this.#queue(this.#saveTitle);
	}

	#saveTitle = async () => {
		const wanted = cleanTitle(this.title);
		if (!wanted) {
			// A page cannot lose its title; a new one waits for one.
			if (this.page) this.title = this.page.subject ?? '';
			return;
		}
		if (this.page?.subject === wanted) {
			this.title = wanted;
			return;
		}
		this.saving = true;
		try {
			if (this.page) {
				const renamed = await renamePage(this.page.id, wanted);
				this.page = { ...renamed, body: this.page.body };
				this.title = renamed.subject ?? wanted;
			} else {
				const text = this.body;
				const created = await createPage(wanted, text, this.#host.date);
				this.page = created;
				this.savedBody = text;
				this.title = created.subject ?? wanted;
				pageDraft.title = '';
				pageDraft.body = '';
				// Leaving the view is what blurred the title: stay left, and
				// the page is done with.
				if (!this.#closed) this.#host.oncreated(created);
				else await finishPage(created.id).catch(() => {});
			}
			this.saved = true;
			this.error = null;
		} catch (e) {
			this.error = { what: m.error_save_page(), detail: String(e) };
		} finally {
			this.saving = false;
		}
	};

	/** The text typed so far is saved first, so none of it stays behind. */
	async move(ask: (page: Note) => void) {
		await this.saveNow();
		// A save that failed says why in the status line, and the page stays.
		if (this.page && this.body.trim() === this.savedBody.trim()) ask(this.page);
	}
}
