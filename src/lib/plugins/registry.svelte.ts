import type { Extension } from '@codemirror/state';
import type { Editor } from './editor';
import type { Note } from '#lib/api.js';
import type {
	ItemView,
	Label,
	MarkdownSyntax,
	NoteChip,
	PluginSettingTab,
	ToolbarButton
} from './api';

/**
 * Everything the plugins add to this window, as they have it now. The hosts
 * (the command center, the editor, the day's header, the dock, the settings)
 * read these lists, so what a plugin adds shows the moment it loads and goes
 * the moment it unloads. Each entry names the plugin it came from.
 */

export interface CommandEntry {
	plugin: string;
	/** `<plugin>:<command>`, unique across plugins. */
	id: string;
	name: Label;
	icon?: string;
	hotkey?: string;
	callback?: () => unknown;
	editorCallback?: (editor: Editor) => unknown;
}

export interface RibbonEntry {
	plugin: string;
	icon: string;
	title: Label;
	callback: () => unknown;
}

export interface ToolbarEntry extends ToolbarButton {
	plugin: string;
}

export interface ViewEntry {
	plugin: string;
	type: string;
	create: () => ItemView;
	/** A page that fills the main area rather than the reading column. */
	fill?: boolean;
}

export interface SettingTabEntry {
	plugin: string;
	tab: PluginSettingTab;
}

export interface SyntaxEntry {
	plugin: string;
	syntax: MarkdownSyntax;
}

export interface EditorExtensionEntry {
	plugin: string;
	extension: Extension;
}

export interface ChipEntry {
	plugin: string;
	chip: (note: Note) => NoteChip | null;
}

interface Lists {
	commands: CommandEntry[];
	ribbon: RibbonEntry[];
	toolbar: ToolbarEntry[];
	/** Panels, docked beside the view. */
	views: ViewEntry[];
	/** Full pages in the main area, each at `/plugin/<type>/`. */
	pages: ViewEntry[];
	settingTabs: SettingTabEntry[];
	syntax: SyntaxEntry[];
	editorExtensions: EditorExtensionEntry[];
	/** Chips under a note's text on its card. */
	chips: ChipEntry[];
}

class Registry implements Lists {
	// Raw: an entry is replaced, never changed in place, so a new array is
	// the one change the hosts need to see.
	commands = $state.raw<CommandEntry[]>([]);
	ribbon = $state.raw<RibbonEntry[]>([]);
	toolbar = $state.raw<ToolbarEntry[]>([]);
	views = $state.raw<ViewEntry[]>([]);
	pages = $state.raw<ViewEntry[]>([]);
	settingTabs = $state.raw<SettingTabEntry[]>([]);
	syntax = $state.raw<SyntaxEntry[]>([]);
	editorExtensions = $state.raw<EditorExtensionEntry[]>([]);
	chips = $state.raw<ChipEntry[]>([]);
}

export const registry = new Registry();

/** Add an entry to a list; returns what takes it away again. */
export function contribute<K extends keyof Lists>(list: K, entry: Lists[K][number]): () => void {
	const lists = registry as Lists;
	lists[list] = [...lists[list], entry] as Lists[K];
	return () => {
		lists[list] = lists[list].filter((e) => e !== entry) as Lists[K];
	};
}

/** A label as it reads now: a function follows the interface language. */
export const labelText = (label: Label) => (typeof label === 'function' ? label() : label);
