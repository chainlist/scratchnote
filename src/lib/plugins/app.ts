import {
	finishPage,
	getDay,
	isPage,
	listDays,
	listPages,
	notesContaining,
	saveNote,
	search,
	updateNote,
	updatePage,
	type Note
} from '$lib/api';
import { attachmentUrl } from '$lib/attachments.svelte';
import { fileName, parseMarkdown, preview } from '$lib/markdown';
import { getLocale } from '$lib/paraglide/runtime';
import type { App } from './api';
import { render } from './render.svelte';

/**
 * The main window's side of `app.workspace`: its layout binds the shell
 * here once it is up. The capture window has none, and the calls do
 * nothing there.
 */
export interface WorkspaceHost {
	readonly day: string;
	openDay(date: string): unknown;
	openCited(note: Pick<Note, 'id' | 'date' | 'kind'>): unknown;
	openPluginPage(type: string, params?: Record<string, string>): unknown;
	openPanel(type: string): void;
	closePanel(type?: string): void;
	/** A page open in the page view or the dock, which releases it on closing. */
	isPageOpen(id: string): boolean;
	/** Load what the views show again. */
	refresh(): Promise<void>;
	/** Say what went wrong, above the view. */
	showError(message: string): void;
}

let host: WorkspaceHost | null = null;

/** Returns what unbinds it. */
export function bindWorkspace(next: WorkspaceHost) {
	host = next;
	return () => {
		if (host === next) host = null;
	};
}

/** A plugin failed at something asked of it: said in the main window, logged elsewhere. */
export function reportError(message: string) {
	console.error(message);
	host?.showError(message);
}

const listeners = new Set<() => void>();

/** Tell the plugins the notes changed: the shell calls it after each reload. */
export function notesChanged() {
	for (const listener of listeners) {
		try {
			listener();
		} catch (e) {
			console.error(e);
		}
	}
}

const runtime = { label: 'main' as App['window'], version: '' };

/** Set once at startup, before any plugin loads. */
export function describeWindow(label: string, version: string) {
	runtime.label = label === 'capture' ? 'capture' : 'main';
	runtime.version = version;
}

/** `app.notes`, saving in the name of `plugin` for the activity log (SPEC 4.11). */
function notes(plugin?: string): App['notes'] {
	return {
		day: getDay,
		days: listDays,
		pages: listPages,
		// Every match, as the plugin API has always given them.
		search: async (query) => (await search(query)).notes,
		containing: notesContaining,
		async setBody(note, body) {
			if (isPage(note)) {
				await updatePage(note.id, body, plugin);
				// A save holds the page, as the page view's saves do. One open
				// in a view is released when that view closes; any other now.
				if (!host?.isPageOpen(note.id)) await finishPage(note.id);
			} else {
				await updateNote(note.date, note.id, body, plugin);
			}
			await host?.refresh();
		},
		async create(body, date) {
			const note = await saveNote(body, date, plugin);
			await host?.refresh();
			return note;
		}
	};
}

const workspace: App['workspace'] = {
	get day() {
		return host?.day ?? '';
	},
	openDay: (date) => void host?.openDay(date),
	openNote: (note) => void host?.openCited(note),
	openPage: (type, params) => void host?.openPluginPage(type, params),
	openView: (type) => host?.openPanel(type),
	closeView: (type) => host?.closePanel(type)
};

const markdown: App['markdown'] = {
	parse: parseMarkdown,
	render,
	images: (text) =>
		preview(parseMarkdown(text), text)
			.attachments.filter((attached) => attached.image)
			.map(({ from, to, name, path }) => ({
				from,
				to,
				name: name || fileName(path),
				url: attachmentUrl(path)
			}))
};

const on: App['on'] = (event, listener) => {
	if (event !== 'notes-changed') throw new Error(`there is no ${event} event`);
	listeners.add(listener);
	return () => listeners.delete(listener);
};

function make(plugin?: string): App {
	return {
		get version() {
			return runtime.version;
		},
		get window() {
			return runtime.label;
		},
		get locale() {
			return getLocale();
		},
		notes: notes(plugin),
		workspace,
		markdown,
		on
	};
}

/** The app as the app itself uses it: what it saves is the user's doing. */
export const app: App = make();

const apps = new Map<string, App>();

/**
 * `this.app` for the plugin `id`, which its views get too: the same app,
 * but what it saves goes on the activity log as the plugin's (SPEC 4.11).
 */
export function appFor(id: string): App {
	let own = apps.get(id);
	if (!own) apps.set(id, (own = make(id)));
	return own;
}
