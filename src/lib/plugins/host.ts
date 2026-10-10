import { errorText } from '#lib/errors.js';
import { app, reportError } from './app';
import type { ViewEntry } from './registry.svelte';
import { onHeaderChange, type ItemView } from './views';

export interface Header {
	title: string;
	icon: string;
	detail?: string;
}

/**
 * Open a plugin's view in `el`, a panel's body or a page's: make it, give it
 * the app, its element and its params, and open it. `onheader` hears its
 * title, icon and detail now and whenever it refreshes them. Returns what
 * closes it.
 */
export function openView(
	entry: ViewEntry,
	el: HTMLElement,
	params: Record<string, string>,
	onheader: (header: Header) => void
): () => void {
	const fail = (e: unknown) => reportError(`${entry.plugin}: ${errorText(e)}`);
	let view: ItemView;
	try {
		view = entry.create();
		view.app = app;
		view.containerEl = el;
		view.params = params;
	} catch (e) {
		fail(e);
		return () => {};
	}
	const header = () => {
		try {
			onheader({ title: view.getDisplayText(), icon: view.getIcon(), detail: view.getDetail() });
		} catch (e) {
			fail(e);
		}
	};
	const stop = onHeaderChange(view, header);
	header();
	try {
		void Promise.resolve(view.onOpen()).then(header, fail);
	} catch (e) {
		fail(e);
	}
	return () => {
		stop();
		try {
			void Promise.resolve(view.onClose()).catch(fail);
		} catch (e) {
			fail(e);
		}
		el.replaceChildren();
	};
}
