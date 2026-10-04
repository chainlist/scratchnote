import type { Extension } from '@codemirror/state';
import type { Tree } from '@lezer/common';
import type { MarkdownExtension } from '@lezer/markdown';
import { pluginData, savePluginData, type Note, type PluginManifest } from '#lib/api.js';
import type { Editor } from './editor';
import { contribute, registry } from './registry.svelte';

export { Editor } from './editor';
export { iconSvg } from './icons';
export {
	ButtonComponent,
	DropdownComponent,
	Setting,
	SettingSection,
	TextComponent,
	ToggleComponent
} from './setting.svelte';
export { Timeline, TimelineItem } from './timeline.svelte';

/**
 * What a plugin is given (SPEC 3.9): `require('scratchnote')` in a
 * community plugin's `main.js`, this module in a core plugin. It follows
 * Obsidian's API, so a plugin for one reads familiar in the other.
 *
 * Plugins outside this repository are written against it: add to it, and
 * keep what is there working. PLUGINS.md is the guide for their authors.
 */

export type { Note, PluginManifest };

/** Text a plugin shows: a string, or a function to follow the interface language. */
export type Label = string | (() => string);

/** The app, as `this.app` in a plugin. */
export interface App {
	/** The app's version, such as `0.3.0`. */
	readonly version: string;
	/**
	 * The window this copy of the plugin runs in. Each window loads its own
	 * copy: the capture window for the editor's syntax and toolbar, the main
	 * window for everything.
	 */
	readonly window: 'main' | 'capture';
	/** The interface language, such as `fr`. */
	readonly locale: string;
	readonly notes: Notes;
	readonly workspace: Workspace;
	readonly markdown: MarkdownRenderer;
	/**
	 * Call `listener` on an event; returns what stops it, for `registerEvent`.
	 * `notes-changed`: a note or page was saved or deleted, here
	 * or on disk, or another space opened.
	 */
	on(event: 'notes-changed', listener: () => void): () => void;
}

/** The notes of the open space. */
export interface Notes {
	/** A day's notes and pages, by time. */
	day(date: string): Promise<Note[]>;
	/**
	 * Every day with notes, newest first: how many notes and pages it has,
	 * and how many words its notes hold, its pages left out.
	 */
	days(): Promise<{ date: string; count: number; words: number }[]>;
	/** Every page, newest first. */
	pages(): Promise<Note[]>;
	/** As the command center searches, by words. Newest first. */
	search(query: string): Promise<Note[]>;
	/** Every note and page whose text holds any of `needles` as typed, newest first. */
	containing(needles: string[]): Promise<Note[]>;
	/** Save a new text for a note or a page. */
	setBody(note: Note, body: string): Promise<void>;
	/** Add a note to a day, today by default. Null for an empty body. */
	create(body: string, date?: string): Promise<Note | null>;
}

/** The main window's views. In the capture window these do nothing. */
export interface Workspace {
	/** The day the main window shows, or went back to last. */
	readonly day: string;
	openDay(date: string): void;
	/** A note on its day, brought into view; a page opens. */
	openNote(note: Pick<Note, 'id' | 'date' | 'kind'>): void;
	/** A page registered with `registerPage`, with its query string. */
	openPage(type: string, params?: Record<string, string>): void;
	/** A view registered with `registerView`, in the dock. */
	openView(type: string): void;
	/** Close the docked view, if it is `type` (or any when left out). */
	closeView(type?: string): void;
}

export interface MarkdownRenderer {
	/** The tree the app reads `text` with, every plugin's syntax included. */
	parse(text: string): Tree;
	/** Draw `text` into `el` as a note's card draws it. */
	render(el: HTMLElement, text: string, options?: RenderOptions): RenderedMarkdown;
	/** The pictures `text` shows: its attached images, in the order they come. */
	images(text: string): MarkdownImage[];
}

/** An attached image a text shows, `![name](path)`. */
export interface MarkdownImage {
	/** Where its markup starts and ends in the text. */
	from: number;
	to: number;
	/** Between the brackets, or the file's name when they are empty. */
	name: string;
	/** Where the webview loads it from, for an `<img>`'s `src`. */
	url: string;
}

export interface RenderOptions {
	/**
	 * A widget changed the text, a task's box ticked: the new text, to save.
	 * Left out, widgets cannot change it.
	 */
	onchange?: (text: string) => void;
	/** Draw links and attachments without opening them, as a preview does. */
	preview?: boolean;
	/** Classes for the text, such as its size. */
	class?: string;
}

export interface RenderedMarkdown {
	update(text: string): void;
	destroy(): void;
}

/** A command in the command center. */
export interface Command {
	/** Unique within the plugin. */
	id: string;
	name: Label;
	icon?: string;
	/** In CodeMirror's notation: `Mod-Shift-h`, where Mod is Ctrl, or Cmd on macOS. */
	hotkey?: string;
	callback?: () => unknown;
	/**
	 * Instead of `callback`, for a command on the text being written: it
	 * shows only when the command center was opened from an editor.
	 */
	editorCallback?: (editor: Editor) => unknown;
}

/** A button on the editor's formatting toolbar. */
export interface ToolbarButton {
	/** Unique within the plugin. */
	id: string;
	icon: string;
	title: Label;
	/** Next to bold and italic, next to the lists, or at the end (the default). */
	group?: 'text' | 'lists' | 'insert';
	run(editor: Editor): void;
	/** Shown pressed while this says so, as bold is in bold text. */
	active?(editor: Editor): boolean;
}

/**
 * Markdown a plugin adds: how it parses, and how the editor and the cards
 * draw it. The editor hides markup away from the line being edited and
 * shows it dimmed there, as it does bold's `**`.
 */
export interface MarkdownSyntax {
	/** A `@lezer/markdown` extension: the nodes it defines and how they parse. */
	extension?: MarkdownExtension;
	/** How to draw each node, by name. */
	render?: Record<string, NodeRender>;
	/**
	 * What a list item may carry after its bullet, as a task's `[ ]`, so
	 * the list buttons keep it with the bullet. Matched right after the
	 * bullet's spaces.
	 */
	itemMarks?: RegExp[];
}

export interface NodeRender {
	/** A class on the node's text, styled by the plugin's `styles.css`. */
	class?: string;
	/** A class on every line the node is on, for a block. */
	line?: string;
	/** The node is markup: hidden off the line being edited. */
	hide?: boolean;
	/** Draw this in the node's place, off the line being edited. */
	widget?: (node: WidgetContext) => HTMLElement;
	/** The spaces after the node go with it, hidden or drawn over. */
	spaces?: boolean;
	/** In a list item, the node takes the bullet's place, as a task's box does. */
	replacesBullet?: boolean;
	/**
	 * The widget handles its own clicks in the editor, as a task's box does.
	 * Otherwise a click on it puts the cursor there, to edit it.
	 */
	handlesClicks?: boolean;
}

/** How a page registered with `registerPage` is laid out. */
export interface PageOptions {
	/**
	 * Fill the main area, its whole width and the height under the title,
	 * rather than the column the notes are read in: for a board, a map or a
	 * book. The view's `containerEl` then has that size, for `height: 100%`.
	 */
	fill?: boolean;
}

/** What a widget is drawn from. */
export interface WidgetContext {
	/** The node's text. */
	text: string;
	/** In the editor, a note's card, or a preview, which should not react. */
	where: 'editor' | 'note' | 'preview';
	/** Whether `update` saves: in the editor, and on a card that saves its note. */
	editable: boolean;
	/** Replace the node's text, as ticking a box replaces `[ ]` with `[x]`. */
	update(text: string): void;
}

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
