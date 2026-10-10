import type { Plugin } from './component';
import type { App } from './types';

const headerListeners = new WeakMap<ItemView, () => void>();

/**
 * A plugin's panel or page. The app sets `app`, `containerEl` and `params`,
 * then calls `onOpen`; `onClose` when it goes.
 */
export class ItemView {
	app!: App;
	/** Where the view draws itself. */
	containerEl!: HTMLElement;
	/** A page's query string, as `openPage` gave it. A new one opens the page anew. */
	params: Record<string, string> = {};

	/** The title over a page, or in a panel's header. */
	getDisplayText(): string {
		return '';
	}

	/** SVG markup, see `icon` in PLUGINS.md. */
	getIcon(): string {
		return '';
	}

	/** Dimmed after a page's title, such as a count. */
	getDetail(): string | undefined {
		return undefined;
	}

	onOpen(): void | Promise<void> {}

	onClose(): void | Promise<void> {}

	/** Read the title, icon and detail again, after they changed. */
	refreshHeader() {
		headerListeners.get(this)?.();
	}
}

/** The host's side of a view: hear when its header changes. */
export function onHeaderChange(view: ItemView, listener: () => void) {
	headerListeners.set(view, listener);
	return () => headerListeners.delete(view);
}

/**
 * A plugin's tab in the settings. `display` fills `containerEl` each time
 * the tab shows, usually with `Setting`s; the app empties it first.
 */
export class PluginSettingTab {
	readonly app: App;
	readonly plugin: Plugin;
	readonly containerEl: HTMLElement = document.createElement('div');
	/** Its icon in the sidebar, SVG markup as `iconSvg` takes; the puzzle piece without. */
	icon?: string;

	constructor(app: App, plugin: Plugin) {
		this.app = app;
		this.plugin = plugin;
	}

	display(): void {}

	/** The tab closed. */
	hide(): void {}
}
