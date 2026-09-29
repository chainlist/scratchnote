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
import { parseMarkdown } from '$lib/markdown';
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

export const app: App = {
	get version() {
		return runtime.version;
	},
	get window() {
		return runtime.label;
	},
	get locale() {
		return getLocale();
	},
	notes: {
		day: getDay,
		days: listDays,
		pages: listPages,
		search,
		containing: notesContaining,
		async setBody(note, body) {
			if (isPage(note)) {
				await updatePage(note.id, body);
				// A save holds the page, as the page view's saves do. One open
				// in a view is released when that view closes; any other now.
				if (!host?.isPageOpen(note.id)) await finishPage(note.id);
			} else {
				await updateNote(note.date, note.id, body);
			}
			await host?.refresh();
		},
		async create(body, date) {
			const note = await saveNote(body, date);
			await host?.refresh();
			return note;
		}
	},
	workspace: {
		get day() {
			return host?.day ?? '';
		},
		openDay: (date) => void host?.openDay(date),
		openNote: (note) => void host?.openCited(note),
		openPage: (type, params) => void host?.openPluginPage(type, params),
		openView: (type) => host?.openPanel(type),
		closeView: (type) => host?.closePanel(type)
	},
	markdown: {
		parse: parseMarkdown,
		render
	},
	on(event, listener) {
		if (event !== 'notes-changed') throw new Error(`there is no ${event} event`);
		listeners.add(listener);
		return () => listeners.delete(listener);
	}
};
