import { Plugin } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import { BOOK, JournalView } from './view';

/** The journal, at `/plugin/journal/`, or `?date=2026-09-29` to open it on a day. */
const PAGE = 'journal';

/**
 * Journal view, a core plugin (SPEC 3.12): the space's days as a journal
 * book, a day to a spread, whose pages turn from one day to the next, with
 * the pictures of the notes taped in.
 */
export class JournalPlugin extends Plugin {
	onload() {
		const open = () => this.app.workspace.openPage(PAGE);
		this.registerPage(PAGE, () => new JournalView(), { fill: true });
		this.addRibbonIcon(BOOK, m.journal_title, open);
		this.addCommand({ id: 'open', name: m.journal_open, icon: BOOK, callback: open });
	}
}
