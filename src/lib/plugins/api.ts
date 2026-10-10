/**
 * What a plugin is given (SPEC 3.9): `require('scratchnote')` in a
 * community plugin's `main.js`, this module in a core plugin. It follows
 * Obsidian's API, so a plugin for one reads familiar in the other.
 *
 * Plugins outside this repository are written against it: add to it, and
 * keep what is there working. PLUGINS.md is the guide for their authors.
 * The app's own side of plugins (loading them, hearing a view's header
 * change) stays in the modules this one gathers from.
 */

export type * from './types';
export { Component, Plugin } from './component';
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
export { ItemView, PluginSettingTab } from './views';
