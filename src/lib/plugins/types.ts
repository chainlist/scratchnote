import type { Tree } from '@lezer/common';
import type { MarkdownExtension } from '@lezer/markdown';
import type { DaySummary, Guess, Note, NoteLabel, PluginManifest } from '#lib/api.js';
import type { Editor } from './editor';

/**
 * The types of the plugin API (SPEC 3.9), which `api.ts` makes public. The
 * app's own modules take them from here, not from `api.ts`, which imports
 * them back.
 */

export type { DaySummary, Guess, Note, NoteLabel, PluginManifest };

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
	 * `meaning-changed`: notes were embedded, so what is read from their
	 * meaning, such as their labels, may have changed.
	 */
	on(event: 'notes-changed' | 'meaning-changed', listener: () => void): () => void;
}

/** The notes of the open space. */
export interface Notes {
	/** A day's notes and pages, by time. */
	day(date: string): Promise<Note[]>;
	/**
	 * Every day with notes, newest first: how many notes and pages it has,
	 * and how many words its notes hold, its pages left out.
	 */
	days(): Promise<DaySummary[]>;
	/** Every page, newest first. */
	pages(): Promise<Note[]>;
	/** As the command center searches, by words. Newest first. */
	search(query: string): Promise<Note[]>;
	/** Every note and page whose text holds any of `needles` as typed, newest first. */
	containing(needles: string[]): Promise<Note[]>;
	/** These notes and pages, in the order asked; one gone is left out. */
	get(ids: string[]): Promise<Note[]>;
	/**
	 * What each note is about, by id, read from its meaning: its part of
	 * life, and for work its job family, each with how sure of it the app
	 * is. Only notes embedded so far, and none without the embedding model.
	 */
	labels(): Promise<Record<string, NoteLabel>>;
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

/** A chip under a note's text on its card, beside its thread. */
export interface NoteChip {
	text: string;
	/** SVG markup, see `icon` in PLUGINS.md. */
	icon?: string;
	/** On pointing at it. */
	title?: string;
	/** Makes it a button. */
	onClick?: () => unknown;
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
