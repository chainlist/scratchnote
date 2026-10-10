import { tick, untrack } from 'svelte';
import { beforeNavigate, pushState } from '$app/navigation';
import { navigating, page } from '$app/state';
import { android } from '#lib/platform.js';

/**
 * Android's back button and gesture go back through the webview's history.
 * While something is open over the views, a dialog, a menu, the dock, the
 * history holds one entry more, of the same view: going back leaves it, and
 * the last thing opened closes rather than the view.
 */

interface Layer {
	close: () => void;
}

/** What is open, the last opened last. */
const layers: Layer[] = [];
let started = false;

/** bits-ui's dialogs, popovers and menus each close on Escape, the top one
 *  only, so Escape is what closes one. */
const escape = () =>
	document.dispatchEvent(
		new KeyboardEvent('keydown', { key: 'Escape', bubbles: true, cancelable: true })
	);

function add(layer: Layer) {
	layers.push(layer);
	void reconcile();
	return () => {
		const at = layers.indexOf(layer);
		if (at >= 0) layers.splice(at, 1);
		void reconcile();
	};
}

/**
 * Keep `close` among what Back closes while `open` holds. Called as a
 * component starts, as an effect is.
 */
export function closeOnBack(open: () => boolean, close: () => void) {
	$effect(() => {
		if (open()) return untrack(() => add({ close }));
	});
}

/** Once no view is loading: every navigation under way has landed or failed. */
export async function afterNavigation() {
	while (navigating.complete) await navigating.complete.catch(() => {});
}

/** Where the history stands: on the entry kept while something is open. */
export const onGuard = () => page.state.overlay === true;

/** Our own step back off the entry, under way, which closes nothing. */
let leaving: (() => void) | null = null;
let busy = false;
/** Counts the views opened, to tell one starting from what just closed. */
let navigations = 0;

/** Add the entry while something is open, or take it away once nothing is,
 *  one step at a time and never while a view is loading. */
async function reconcile() {
	if (!started || busy) return;
	busy = true;
	try {
		for (;;) {
			await tick();
			await afterNavigation();
			const want = layers.length > 0;
			if (want === onGuard()) return;
			if (want) {
				await pushState('', { overlay: true });
				continue;
			}
			// What closed may be opening a view, as a note picked in the command
			// center does, which starts loading a moment later: going back then
			// would cancel it. The entry is left behind the new view instead,
			// and stepped over once Back reaches it.
			const since = navigations;
			await new Promise((resolve) => setTimeout(resolve, 100));
			if (navigations !== since || navigating.to || layers.length) continue;
			await new Promise<void>((resolve) => {
				leaving = resolve;
				history.back();
				// Never stuck, should the step back not come.
				setTimeout(resolve, 1000);
			});
		}
	} finally {
		busy = false;
	}
}

/**
 * Watch the history from the views' layout, on Android only. bits-ui keeps
 * its open layers in a global map, by the order they opened; it is wrapped
 * so each one opening or closing is heard.
 */
export function startBackGuard() {
	if (!android) return;
	started = true;
	beforeNavigate(() => void navigations++);

	// eslint-disable-next-line svelte/prefer-svelte-reactivity -- nothing is drawn from it
	const bits = new Map<unknown, Layer>();
	class Watched extends Map<unknown, unknown> {
		override set(key: unknown, value: unknown) {
			if (!bits.has(key)) {
				const layer = { close: escape };
				bits.set(key, layer);
				// Not through `add`'s remover, as the map can take it out itself.
				add(layer);
			}
			return super.set(key, value);
		}
		override delete(key: unknown) {
			const layer = bits.get(key);
			if (layer) {
				bits.delete(key);
				const at = layers.indexOf(layer);
				if (at >= 0) layers.splice(at, 1);
				void reconcile();
			}
			return super.delete(key);
		}
	}
	const global = globalThis as { bitsEscapeLayers?: Map<unknown, unknown> };
	global.bitsEscapeLayers = new Watched(global.bitsEscapeLayers ?? []);

	// The entry left by going back, on the same view: Back was pressed, or
	// we stepped back ourselves.
	let wasGuard = onGuard();
	let lastHref = page.url.href;
	$effect(() => {
		const guard = onGuard();
		const href = page.url.href;
		untrack(() => {
			const popped = wasGuard && !guard && href === lastHref;
			wasGuard = guard;
			lastHref = href;
			if (popped && leaving) {
				leaving();
				leaving = null;
			} else if (popped) {
				layers.at(-1)?.close();
			}
			void reconcile();
		});
	});
}
