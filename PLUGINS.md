# Writing a Scratchnote plugin

A plugin adds to Scratchnote the way an Obsidian plugin adds to Obsidian:
markdown syntax for the editor and the cards, commands in the command center,
buttons on the editor's toolbar and down the left edge of the window, a panel
docked beside the day, pages of its own, and a settings tab. The API follows
Obsidian's, so an Obsidian plugin's author will find their way around.

A plugin is trusted code. It runs in the app's webview with the app's own
access, which is why community plugins stay off until the user turns them on
(SPEC 3.9). The reference for the API is
[`src/lib/plugins/api.ts`](src/lib/plugins/api.ts); the core plugins in
[`src/plugins/`](src/plugins/) use most of it.

## The files

A plugin is a folder named after its id:

```
highlights/
  manifest.json
  main.js
  styles.css      (optional)
```

`manifest.json`, as Obsidian's:

```json
{
	"id": "highlights",
	"name": "Highlights",
	"version": "1.0.0",
	"minAppVersion": "0.2.0",
	"description": "Highlight text with ==double equals==.",
	"author": "Your name",
	"authorUrl": "https://github.com/you"
}
```

The id is lowercase letters, digits and dashes. `minAppVersion` is the oldest
Scratchnote the plugin runs on.

`main.js` is a CommonJS module whose export is the plugin's class. `styles.css`
is added to the page while the plugin is on.

## A first plugin

```js
const { Plugin } = require('scratchnote');

class HelloPlugin extends Plugin {
	onload() {
		this.addCommand({
			id: 'hello',
			name: 'Write hello',
			callback: () => this.app.notes.create('Hello from a plugin')
		});
	}
}

module.exports = HelloPlugin;
```

To try it:

1. Put the folder in the notes root's `.scratchnote/plugins/`, so
   `~/Scratchnote/.scratchnote/plugins/hello/`.
2. In Settings > Community plugins, turn community plugins on, and switch
   Hello on in Installed plugins.
3. After changing `main.js`, switch the plugin off and on: it is read again.

Errors go to the webview's console, where `main.js` shows as
`plugins/<id>/main.js`. A dev build opens the developer tools with a right
click and Inspect.

## What `require` gives

| Module                 | What it is                                                                                                          |
| ---------------------- | ------------------------------------------------------------------------------------------------------------------- |
| `scratchnote`          | `Plugin`, `Component`, `ItemView`, `PluginSettingTab`, `Setting`, `SettingSection`, `Timeline`, `Editor`, `iconSvg` |
| `@codemirror/state`    | the editor's own copy                                                                                               |
| `@codemirror/view`     | the editor's own copy                                                                                               |
| `@codemirror/language` | the editor's own copy                                                                                               |
| `@lezer/common`        | the parser's own copy                                                                                               |
| `@lezer/markdown`      | the parser's own copy                                                                                               |

These are the app's copies, so a syntax extension or an editor extension is
made of the same classes as the editor's. Anything else a plugin uses, it
bundles into `main.js`, as Obsidian plugins do: esbuild with those modules
marked external works.

## Lifecycle

- The app constructs the class with `(app, manifest)` and calls `onload`. An
  async `onload` is waited for two seconds at most; register syntax before the
  first `await`, so the first card is drawn with it.
- Everything added through the plugin's methods (commands, buttons, views,
  syntax, settings tabs, and whatever is passed to `register`) goes when the
  plugin unloads: switched off, updated or uninstalled. `onunload` is for the
  rest. A part of the plugin can come and go by itself, see [Parts](#parts).
- Each window loads its own copy: the main window, and the capture window for
  the editor's syntax and toolbar. `this.app.window` says which. In the capture
  window, `app.workspace` does nothing.
- `onExternalSettingsChange` runs when another window saved the plugin's data,
  so its copy there can read it again.

## The plugin's methods

| Method                                     | What it adds                                                      |
| ------------------------------------------ | ----------------------------------------------------------------- |
| `addCommand(command)`                      | a command in the command center, with an optional hotkey          |
| `addToolbarButton(button)`                 | a button on the editor's formatting toolbar                       |
| `addRibbonIcon(icon, title, callback)`     | a button down the left edge of the window, below the app's own    |
| `addSettingTab(tab)`                       | a tab in the settings, under Community plugins                    |
| `registerView(type, create)`               | a panel in the dock, opened with `app.workspace.openView`         |
| `registerPage(type, create, options?)`     | a page at `/plugin/<type>/`, opened with `app.workspace.openPage` |
| `registerMarkdownSyntax(syntax)`           | syntax for the editor and the cards                               |
| `registerEditorExtension(extension)`       | any CodeMirror extension, in every editor                         |
| `loadData()`, `saveData(data)`             | the plugin's `data.json`                                          |
| `register(cleanup)`                        | something to undo on unload                                       |
| `registerEvent(off)`                       | `this.registerEvent(this.app.on('notes-changed', ...))`           |
| `registerDomEvent(target, type, listener)` | a DOM listener removed on unload                                  |
| `registerInterval(id)`                     | an interval cleared on unload                                     |
| `addChild(component)`, `removeChild(c)`    | a part with a life of its own, see [Parts](#parts)                |

All but `loadData` and `saveData` are `Component`'s, which `Plugin` extends.
Names and titles are strings, or functions returning one, which a core plugin
uses to follow the interface language.

### Parts

A plugin that brings several things the user switches on and off one by one
makes each a `Component`. `addChild` loads it with the plugin, or at once if
the plugin is loaded; `removeChild` unloads it, and everything it added goes
with it, while the rest of the plugin stays. A component has the plugin's
methods, and adds for the plugin it is a child of.

```js
const { Component, Plugin } = require('scratchnote');

class Highlights extends Component {
	onload() {
		this.registerMarkdownSyntax(highlightSyntax);
		this.addCommand({ id: 'highlight', name: 'Highlight' /* ... */ });
	}
}

class MyPlugin extends Plugin {
	async onload() {
		this.settings = { highlights: true, ...(await this.loadData()) };
		if (this.settings.highlights) this.highlights = this.addChild(new Highlights());
	}

	/** From its settings tab's switch. */
	async setHighlights(on) {
		if (on && !this.highlights) this.highlights = this.addChild(new Highlights());
		if (!on && this.highlights) this.highlights = void this.removeChild(this.highlights);
		this.settings.highlights = on;
		await this.saveData(this.settings);
	}
}
```

The Basics core plugin is built this way: Tasks and Highlights are its parts,
in [`src/plugins/basics/`](src/plugins/basics/).

### Commands

```js
this.addCommand({
	id: 'toggle',
	name: 'Highlight',
	icon: HIGHLIGHTER,
	hotkey: 'Mod-Shift-h',
	editorCallback: (editor) => editor.toggleMark('Highlight', '==')
});
```

A command has a `callback`, or an `editorCallback` for one on the text being
written: that one shows only when the command center was opened from an
editor, and gets the `Editor`. Hotkeys are in CodeMirror's notation, `Mod` being
Ctrl, or Cmd on macOS. A hotkey on the text works in any editor, the others in
the main window.

### Toolbar buttons

```js
this.addToolbarButton({
	id: 'toggle',
	icon: HIGHLIGHTER,
	title: 'Highlight',
	group: 'text',
	run: (editor) => editor.toggleMark('Highlight', '=='),
	active: (editor) => editor.hasMark('Highlight', '==')
});
```

`group` puts it beside bold and italic (`text`), beside the lists (`lists`), or
at the end (`insert`, the default). With `active`, the button shows pressed
while it returns true.

### The editor

`Editor` does what the app's own toolbar does, so a plugin's format behaves as
bold or a list does:

| Method                                     | What it does                                                          |
| ------------------------------------------ | --------------------------------------------------------------------- |
| `getValue()`, `setValue(text)`             | the whole text                                                        |
| `getSelection()`, `replaceSelection(text)` | the selection                                                         |
| `toggleMark(node, mark)`                   | `mark` around the selection or the word, or off where the text has it |
| `hasMark(node, mark)`                      | whether the selection has that format                                 |
| `toggleList(mark, itemMark?)`              | the selected lines as a list whose items start with `mark`, or plain  |
| `isList(itemMark?)`                        | whether every selected line is such an item                           |
| `cm`                                       | the CodeMirror `EditorView`, for anything else                        |

`node` is what the format parses to, the node the plugin's syntax defines,
opened and closed by its first and last child.

### Markdown syntax

A syntax is a `@lezer/markdown` extension and how to draw its nodes. This is
the Basics core plugin's Highlights, in short:

```js
const DELIMITER = { resolve: 'Highlight', mark: 'HighlightMark' };

this.registerMarkdownSyntax({
	extension: {
		defineNodes: ['Highlight', 'HighlightMark'],
		parseInline: [
			{
				name: 'Highlight',
				parse(cx, next, pos) {
					if (next !== 61 || cx.char(pos + 1) !== 61) return -1;
					return cx.addDelimiter(DELIMITER, pos, pos + 2, true, true);
				},
				after: 'Emphasis'
			}
		]
	},
	render: {
		Highlight: { class: 'hl-mark' },
		HighlightMark: { hide: true }
	}
});
```

Each node name maps to how it is drawn:

| Rule             | What it does                                                                |
| ---------------- | --------------------------------------------------------------------------- |
| `class`          | a class on the node's text, styled by `styles.css`                          |
| `line`           | a class on every line the node is on, for a block                           |
| `hide`           | markup: hidden, and shown dimmed on the line being edited, as bold's `**`   |
| `widget(node)`   | an element drawn in the node's place, off the line being edited             |
| `spaces`         | the spaces after the node go with it                                        |
| `replacesBullet` | in a list item, the node takes the bullet's place, as a task's box does     |
| `handlesClicks`  | the widget handles its own clicks in the editor; otherwise a click edits it |

A widget gets `{ text, where, editable, update }`: the node's text, where it is
drawn (`editor`, `note` or `preview`, which should not react to clicks),
whether `update(text)` saves, and `update`, which replaces the node's text, as
ticking a box replaces `[ ]` with `[x]`. A task's box in the Basics core
plugin, in [`src/plugins/basics/tasks/syntax.ts`](src/plugins/basics/tasks/syntax.ts),
is a widget.

`itemMarks` names what a list item can carry after its bullet, as a task's
`[ ]`, so the list buttons keep it with the bullet.

The editor and the cards redraw with a syntax as soon as it is registered, and
without it once it goes. The file keeps what was typed.

### Views and pages

A view and a page are both an `ItemView`:

```js
class StatsView extends ItemView {
	getDisplayText() {
		return 'Space stats';
	}
	getIcon() {
		return CHART;
	}
	async onOpen() {
		const days = await this.app.notes.days();
		this.containerEl.textContent = `${days.length} days written`;
	}
	onClose() {}
}

this.registerView('space-stats', () => new StatsView());
this.app.workspace.openView('space-stats');
```

`registerView` makes it a panel in the dock, where a page docks;
`registerPage` makes it a page in the main area at `/plugin/<type>/`, with its
title and a way back to the day. A page gets its query string as
`this.params`: `app.workspace.openPage('mentions', { name: 'marie' })` opens
`/plugin/mentions/?name=marie`, and another query opens it anew.
`getDetail()` is dimmed after a page's title, as a count; call
`refreshHeader()` when the title, icon or detail change.

A page is drawn in the column the notes are read in. One that needs the room,
such as a board or a book, fills the main area instead, its whole width and
the height under its title: `registerPage(type, create, { fill: true })`. Its
`containerEl` then has that size, so `height: 100%` inside it fills the window,
and `container-type: size` lets its styles size things by it, as the Journal
view core plugin sizes its book.

### Timelines

A list of notes, or of anything that happened at a time, is drawn on the
day's own timeline: the time on the left, the rail with its dot, and the body
on the right. The app draws the time and the rail with the day's component,
so a plugin's timeline looks like the day's and follows it when the day's
design changes. The body is the plugin's element, `contentEl`, to fill with
markdown, plain DOM, or a root of another framework, such as React's
`createRoot(item.contentEl)`.

```js
const { Timeline } = require('scratchnote');

const timeline = new Timeline(this.containerEl);
for (const note of notes) {
	const item = timeline.addItem((item) =>
		item
			.setDate(note.date)
			.setTime(note.time)
			.setIcon(note.kind === 'page' ? FILE : undefined)
			.onClickTime(() => this.app.workspace.openNote(note))
	);
	this.drawn.push(this.app.markdown.render(item.contentEl, note.body));
}
```

| Method                       | What it does                                                  |
| ---------------------------- | ------------------------------------------------------------- |
| `addItem(build?)`            | an item at the end; `build` sets it up, and it is returned    |
| `clear()`                    | every item taken away                                         |
| `timelineEl`                 | the list                                                      |
| `item.setTime(time)`         | the time on the left, such as `14:05`                         |
| `item.setDate(date?)`        | the date above it, for a timeline that spans days             |
| `item.setIcon(icon?)`        | an icon in a box on the rail, as a page has; the dot without  |
| `item.onClickTime(callback)` | the date and time as a button, such as to the note on its day |
| `item.remove()`              | the item taken away                                           |
| `item.contentEl`             | the body, the plugin's                                        |
| `item.itemEl`                | the item's `li`                                               |

The dot sits level with a first line of text at `1rem` on `1.75rem`, as a
note's is. An item's setters can be called again, and the item redraws.

Call `clear()` (or `remove()`) before emptying the element the timeline is in,
as when a view closes, and destroy what you drew in the items first: an item
left mounted stays in memory, as a `markdown.render` left undestroyed does.

### Settings

```js
class HighlightsSettingTab extends PluginSettingTab {
	display() {
		new Setting(this.containerEl)
			.setName('Colour')
			.setDesc('What highlighted text is painted with.')
			.addDropdown((dropdown) =>
				dropdown
					.addOptions({ yellow: 'Yellow', pink: 'Pink' })
					.setValue(this.plugin.settings.colour)
					.onChange(async (colour) => {
						this.plugin.settings.colour = colour;
						await this.plugin.saveData(this.plugin.settings);
					})
			);
	}
}

this.addSettingTab(new HighlightsSettingTab(this.app, this));
```

`display` fills `containerEl` each time the tab shows; the app empties it
first. A `Setting` is drawn with the app's own switches, fields and buttons:
`addToggle`, `addText`, `addDropdown`, `addButton`, and `setHeading` for a
title between boxes of rows, which takes a description and controls too.
`icon` on the tab sets its icon in the sidebar.

A tab that holds several things puts each in a `SettingSection`: a title, a
line under it, and its settings in a box of their own. With a switch, as a
part the user switches on and off has, its settings show only while it is on.

```js
const section = new SettingSection(this.containerEl)
	.setName('Highlights')
	.setDesc('Text between ==double equals==.')
	.addToggle((toggle) =>
		toggle.setValue(this.plugin.settings.highlights).onChange((on) => this.plugin.setHighlights(on))
	);
new Setting(section.contentEl).setName('Colour').addDropdown(/* ... */);
```

`contentEl` is where its settings go, and `sectionEl` the whole section.

### The app

`this.app`:

| Member                                                                    | What it is                                                     |
| ------------------------------------------------------------------------- | -------------------------------------------------------------- |
| `version`, `window`, `locale`                                             | the app's version, this copy's window, the interface language  |
| `notes.day(date)`, `notes.days()`, `notes.pages()`                        | the open space's notes                                         |
| `notes.search(query)`                                                     | as the command center searches: words and `#category`          |
| `notes.containing(needles)`                                               | every note and page whose text holds any of `needles`          |
| `notes.setBody(note, body)`                                               | save a note's or a page's new text                             |
| `notes.create(body, date?)`                                               | add a note to a day                                            |
| `workspace.day`, `openDay(date)`, `openNote(note)`                        | the day shown, and the way to a day or a note                  |
| `workspace.openPage(type, params?)`, `openView(type)`, `closeView(type?)` | a plugin's page or panel                                       |
| `markdown.parse(text)`                                                    | the syntax tree, every plugin's syntax included                |
| `markdown.render(el, text, options?)`                                     | draw markdown as a card does; `onchange` lets widgets save     |
| `markdown.images(text)`                                                   | the attached images a text shows: where, name, and `url`       |
| `on('notes-changed', listener)`                                           | a note was saved, labelled or deleted, or another space opened |

### Icons

An icon is SVG markup: a whole `<svg>`, or only what goes inside a 24 by 24
Lucide icon, which is drawn with Lucide's stroke, so it sits with the app's
own. Copy an icon's markup from [lucide.dev](https://lucide.dev).

### Styling

The app is styled with Tailwind, whose classes a community plugin cannot rely
on: only the classes the app itself uses are built. Style with `styles.css`
and the app's CSS variables, which follow the theme and the accent colour:

| Variable                                      | What it is                                                                             |
| --------------------------------------------- | -------------------------------------------------------------------------------------- |
| `--primary`, `--primary-foreground`           | the accent, and text on it                                                             |
| `--color-neutral-50` to `--color-neutral-950` | the greys, which turn over in light mode, so `--color-neutral-200` is the text in both |
| `--border`, `--card`, `--muted-foreground`    | borders, card backgrounds, dim text                                                    |
| `--font-mono`                                 | the monospace font                                                                     |

A note waiting for the model draws a glowing copy of its text, inside
`.note-glow-text`; a plugin's backgrounds and borders there should go
(`background: none`).

## Publishing

Community plugins come from GitHub, as Obsidian's do (SPEC 4.10):

1. Give the plugin a repository with `manifest.json` and `README.md` at its
   root. The README shows in the plugin's details, before it is installed.
2. For each version, make a GitHub release tagged with the version, such as
   `1.0.0`, and attach `manifest.json`, `main.js` and `styles.css`.
3. Add the plugin to `community-plugins.json` in
   `chainlist/scratchnote-plugins`: its id, name, author, description and
   repository.

Updates are new releases with a newer version in the repository's
`manifest.json`; Check for updates in Settings > Community plugins
finds them.

To test before publishing, lay out a folder as the registry does and point
`SCRATCHNOTE_PLUGIN_REGISTRY` at it, or add the plugin to this repository's
`plugin-registry/`, which a dev build reads.

## Core plugins

A core plugin is a folder of [`src/plugins/`](src/plugins/), listed in
`src/plugins/index.ts`. It uses the same API, from `$lib/plugins/api`, and
loads the same way; it may be written in TypeScript and Svelte (mounting its
components into the element it is given), use the app's messages and UI
components, and nothing else of the app. What a core plugin does, a community
plugin can do. Its data goes in `core-plugins/<id>.json`.

A core plugin is on unless the user switches it off. One with
`offByDefault: true` in its entry starts off instead, until the user switches
it on, as Mentions, Stats and Journal view do.
