import { mount, unmount } from 'svelte';
import { Component, ItemView } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import type { BasicsPlugin } from '../plugin.svelte';
import TasksList from './TasksList.svelte';
import { taskSyntax } from './syntax';
import { BOX } from './tasks';

/** Lucide's list-todo. */
export const LIST_TODO =
	'<path d="M13 5h8"/><path d="M13 12h8"/><path d="M13 19h8"/><path d="m3 17 2 2 4-4"/><rect x="3" y="4" width="6" height="6" rx="1"/>';

/** The page of open tasks, at `/plugin/tasks/`. */
const PAGE = 'tasks';

/**
 * Tasks, a part of Basics (SPEC 3.8): the `- [ ]` syntax and its boxes, the
 * checklist button, and the open tasks of the space on a page of their own.
 * Switched off, `- [ ]` shows as typed.
 */
export class Tasks extends Component {
	#plugin: BasicsPlugin;
	#ribbon: { remove(): void } | null = null;

	constructor(plugin: BasicsPlugin) {
		super();
		this.#plugin = plugin;
	}

	onload() {
		this.registerMarkdownSyntax(taskSyntax);
		this.addToolbarButton({
			id: 'checklist',
			icon: LIST_TODO,
			title: m.format_checklist,
			group: 'lists',
			run: (editor) => editor.toggleList('- [ ] ', BOX),
			active: (editor) => editor.isList(BOX)
		});
		this.registerPage(PAGE, () => new TasksView(this.#plugin));
		this.addCommand({
			id: 'open-tasks',
			name: m.tasks_open,
			icon: LIST_TODO,
			callback: () => this.#open()
		});
		this.update();
	}

	onunload() {
		// The button goes with everything else the part added.
		this.#ribbon = null;
	}

	/** The button on the left edge, unless the settings take it away. */
	update() {
		const wanted = this.#plugin.settings.tasks.ribbon;
		if (wanted && !this.#ribbon) {
			this.#ribbon = this.addRibbonIcon(LIST_TODO, m.tasks_open, () => this.#open());
		} else if (!wanted && this.#ribbon) {
			this.#ribbon.remove();
			this.#ribbon = null;
		}
	}

	#open() {
		this.#plugin.app.workspace.openPage(PAGE);
	}
}

/** The open tasks, with their count after the title. */
class TasksView extends ItemView {
	#plugin: BasicsPlugin;
	#count = 0;
	#list: ReturnType<typeof mount> | null = null;

	constructor(plugin: BasicsPlugin) {
		super();
		this.#plugin = plugin;
	}

	getDisplayText() {
		return m.tasks_open();
	}

	getIcon() {
		return LIST_TODO;
	}

	getDetail() {
		return this.#count ? String(this.#count) : undefined;
	}

	onOpen() {
		this.#list = mount(TasksList, {
			target: this.containerEl,
			props: {
				plugin: this.#plugin,
				oncount: (count: number) => {
					this.#count = count;
					this.refreshHeader();
				}
			}
		});
	}

	onClose() {
		if (this.#list) void unmount(this.#list);
		this.#list = null;
	}
}
