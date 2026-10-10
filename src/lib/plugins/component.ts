import type { Extension } from '@codemirror/state';
import { pluginData, savePluginData } from '#lib/api.js';
import { contribute, registry } from './registry.svelte';
import type {
	App,
	Command,
	Label,
	MarkdownSyntax,
	Note,
	NoteChip,
	PageOptions,
	PluginManifest,
	ToolbarButton
} from './types';
import type { ItemView, PluginSettingTab } from './views';

/**
 * A plugin's life: `Component` and `Plugin` as plugins see them, and the
 * loader's side of them, which loads and unloads a plugin with its children.
 */

interface Internals {
	/** A core plugin keeps its data with the app's (SPEC 4.9). */
	core: boolean;
	/** From its `onload` until it unloads. */
	loaded: boolean;
	/** What it was added to with `addChild`. */
	parent: Component | null;
	children: Component[];
	cleanups: (() => void)[];
}

const internals = new WeakMap<Component, Internals>();
/** Set while the loader constructs a core plugin. */
let constructingCore = false;

const inner = (component: Component) => internals.get(component)!;

/** The plugin a component adds to the app for: itself, or the one it is a child of. */
function pluginOf(component: Component): Plugin {
	for (let at: Component | null = component; at; at = inner(at).parent) {
		if (at instanceof Plugin) return at;
	}
	throw new Error('a component adds to the app once it is a child of a plugin');
}

/** Its `onload`, then its children's. Returns what `onload` did. */
function begin(component: Component): void | Promise<void> {
	const self = inner(component);
	self.loaded = true;
	const result = component.onload();
	for (const child of self.children) loadChild(child);
	return result;
}

/** A child that fails to load is reported, and the rest of the plugin carries on. */
function loadChild(child: Component) {
	if (inner(child).loaded) return;
	const fail = (e: unknown) => console.error(`${pluginOf(child).manifest.id}: a part failed`, e);
	try {
		const result = begin(child);
		if (result instanceof Promise) result.catch(fail);
	} catch (e) {
		fail(e);
	}
}

/** Its own `onunload`, its children, then everything it registered, newest first. */
function end(component: Component) {
	const self = inner(component);
	if (!self.loaded) return;
	self.loaded = false;
	const id = pluginOf(component).manifest.id;
	try {
		component.onunload();
	} catch (e) {
		console.error(`${id}: onunload failed`, e);
	}
	for (const child of [...self.children].reverse()) end(child);
	for (const cleanup of self.cleanups.splice(0).reverse()) {
		try {
			cleanup();
		} catch (e) {
			console.error(`${id}: a cleanup failed`, e);
		}
	}
}

/**
 * Something with a life of its own, as Obsidian's `Component`: everything
 * it adds through these methods goes when it unloads. A `Plugin` is one. A
 * part of a plugin that comes and goes by itself, such as a feature the
 * user switches on and off, is another: `addChild` loads it with the
 * plugin, or at once if the plugin is loaded, and `removeChild` takes it
 * away with all it added, leaving the rest of the plugin as it was.
 */
export class Component {
	constructor() {
		internals.set(this, { core: false, loaded: false, parent: null, children: [], cleanups: [] });
	}

	/** Add what it brings. */
	onload(): void | Promise<void> {}

	/** Stop anything `register` does not know of. */
	onunload(): void {}

	/** `child` lives inside this one, loaded and unloaded with it. Returns `child`. */
	addChild<T extends Component>(child: T): T {
		if (inner(child).parent) throw new Error('the component is a child already');
		inner(child).parent = this;
		inner(this).children.push(child);
		if (inner(this).loaded) loadChild(child);
		return child;
	}

	/** Unload `child`, and everything it added. Returns `child`. */
	removeChild<T extends Component>(child: T): T {
		const children = inner(this).children;
		const at = children.indexOf(child);
		if (at < 0) return child;
		children.splice(at, 1);
		end(child);
		inner(child).parent = null;
		return child;
	}

	/** Run `cleanup` when it unloads. */
	register(cleanup: () => void) {
		inner(this).cleanups.push(cleanup);
	}

	/** Stop listening when it unloads: `registerEvent(app.on(...))`. */
	registerEvent(off: () => void) {
		this.register(off);
	}

	registerDomEvent<K extends keyof WindowEventMap>(
		target: Window | Document | HTMLElement,
		type: K,
		listener: (event: WindowEventMap[K]) => void,
		options?: AddEventListenerOptions
	) {
		target.addEventListener(type, listener as EventListener, options);
		this.register(() => target.removeEventListener(type, listener as EventListener, options));
	}

	registerInterval(id: number): number {
		this.register(() => clearInterval(id));
		return id;
	}

	addCommand(command: Command): Command {
		const { id, name, icon, hotkey, callback, editorCallback } = command;
		const plugin = pluginOf(this).manifest.id;
		this.register(
			contribute('commands', {
				plugin,
				id: `${plugin}:${id}`,
				name,
				icon,
				hotkey,
				callback,
				editorCallback
			})
		);
		return command;
	}

	/**
	 * A button down the left edge of the window, in every view: a core
	 * plugin's with the calendar and All pages, a community plugin's below
	 * them, past a line. Returns what takes it away before it unloads.
	 */
	addRibbonIcon(icon: string, title: Label, callback: () => unknown): { remove(): void } {
		const plugin = pluginOf(this).manifest.id;
		const remove = contribute('ribbon', { plugin, icon, title, callback });
		this.register(remove);
		return { remove };
	}

	addToolbarButton(button: ToolbarButton) {
		this.register(contribute('toolbar', { ...button, plugin: pluginOf(this).manifest.id }));
	}

	/** A tab of the settings, listed under the plugin's name in the sidebar. */
	addSettingTab(tab: PluginSettingTab) {
		this.register(contribute('settingTabs', { plugin: pluginOf(this).manifest.id, tab }));
	}

	/** A panel, docked beside the view by `workspace.openView(type)`. */
	registerView(type: string, create: () => ItemView) {
		if (registry.views.some((view) => view.type === type))
			throw new Error(`a view named ${type} is registered already`);
		this.register(contribute('views', { plugin: pluginOf(this).manifest.id, type, create }));
	}

	/**
	 * A page in the main area, at `/plugin/<type>/`, with a way back to the
	 * day: `workspace.openPage(type)` opens it.
	 */
	registerPage(type: string, create: () => ItemView, options: PageOptions = {}) {
		if (registry.pages.some((page) => page.type === type))
			throw new Error(`a page named ${type} is registered already`);
		const fill = options.fill ?? false;
		this.register(contribute('pages', { plugin: pluginOf(this).manifest.id, type, create, fill }));
	}

	/**
	 * A chip on a note's card, from what `chip` returns for the note, or none
	 * for null. The cards call it as they draw, so it should only read what
	 * the plugin holds; one reading Svelte state redraws as that changes.
	 */
	registerNoteChip(chip: (note: Note) => NoteChip | null) {
		this.register(contribute('chips', { plugin: pluginOf(this).manifest.id, chip }));
	}

	/** Syntax for the editor and the cards; they redraw with it at once. */
	registerMarkdownSyntax(syntax: MarkdownSyntax) {
		this.register(contribute('syntax', { plugin: pluginOf(this).manifest.id, syntax }));
	}

	/** Any CodeMirror extension, added to every editor. */
	registerEditorExtension(extension: Extension) {
		this.register(
			contribute('editorExtensions', { plugin: pluginOf(this).manifest.id, extension })
		);
	}
}

/**
 * A plugin's main class, which its `main.js` exports. Everything it adds
 * through these methods, and its children with theirs, goes when it
 * unloads: switched off, updated, or uninstalled.
 */
export class Plugin extends Component {
	readonly app: App;
	readonly manifest: PluginManifest;

	constructor(app: App, manifest: PluginManifest) {
		super();
		this.app = app;
		this.manifest = manifest;
		inner(this).core = constructingCore;
	}

	/** Add what the plugin brings. The app waits a moment for it, not longer. */
	onload(): void | Promise<void> {}

	/** Another window saved this plugin's data, from its settings: read it again. */
	onExternalSettingsChange(): void {}

	/** What `saveData` saved last, or null before it ever did. */
	async loadData(): Promise<unknown> {
		return pluginData(this.manifest.id, inner(this).core);
	}

	/** Kept in the notes root's `.scratchnote/`, with the plugin (SPEC 4.9). */
	async saveData(data: unknown): Promise<void> {
		await savePluginData(this.manifest.id, inner(this).core, data);
	}
}

/** The loader's side of a plugin: build it, and take everything it added away. */
export function createPlugin(
	Class: new (app: App, manifest: PluginManifest) => Plugin,
	app: App,
	manifest: PluginManifest,
	core: boolean
): Plugin {
	constructingCore = core;
	try {
		return new Class(app, manifest);
	} finally {
		constructingCore = false;
	}
}

/** Load a plugin: its `onload`, which the loader may wait for, and its children. */
export const loadPlugin = (plugin: Plugin) => begin(plugin);

/** Unload a plugin: its own `onunload`, its children, then everything it registered, newest first. */
export const unloadPlugin = (plugin: Plugin) => end(plugin);
