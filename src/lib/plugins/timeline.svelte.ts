import { mount, unmount } from 'svelte';
import TimelineEntry from './TimelineEntry.svelte';

/**
 * A timeline drawn as the day draws its notes: the time on the left, the
 * rail with a dot or an icon, and the body on the right, which is the
 * plugin's own element to fill with markdown, a React root or plain DOM.
 * The app draws the rest with the day's own component, so a plugin's
 * timeline looks like the day's and changes with it.
 *
 * ```js
 * const timeline = new Timeline(containerEl);
 * const item = timeline.addItem((item) =>
 *   item.setDate(note.date).setTime(note.time).onClickTime(() => app.workspace.openNote(note))
 * );
 * app.markdown.render(item.contentEl, note.body);
 * ```
 */

export interface TimelineItemModel {
	time: string;
	date?: string;
	icon?: string;
	click?: () => unknown;
	clickTitle?: string;
}

export class Timeline {
	/** The list the items are in. */
	readonly timelineEl: HTMLUListElement;
	#items: TimelineItem[] = [];

	constructor(containerEl: HTMLElement) {
		this.timelineEl = document.createElement('ul');
		containerEl.append(this.timelineEl);
	}

	/** An item at the end of the timeline, set up by `build` as a `Setting`'s controls are. */
	addItem(build?: (item: TimelineItem) => unknown): TimelineItem {
		const item = new TimelineItem(this.timelineEl, () => {
			this.#items = this.#items.filter((other) => other !== item);
		});
		this.#items.push(item);
		build?.(item);
		return item;
	}

	/** Take every item away. What the plugin drew in them is its to destroy first. */
	clear() {
		for (const item of this.#items) item.remove();
	}
}

export class TimelineItem {
	/** The item's `li`. */
	readonly itemEl: HTMLLIElement;
	/** The body, right of the rail: the plugin's to fill. */
	readonly contentEl: HTMLDivElement;
	#model: TimelineItemModel = $state({ time: '' });
	#drawn: ReturnType<typeof mount> | null;
	#onremove: () => void;

	constructor(timelineEl: HTMLElement, onremove: () => void) {
		this.itemEl = document.createElement('li');
		this.contentEl = document.createElement('div');
		timelineEl.append(this.itemEl);
		this.#drawn = mount(TimelineEntry, {
			target: this.itemEl,
			props: { model: this.#model, contentEl: this.contentEl }
		});
		this.#onremove = onremove;
	}

	/** Such as `14:05`, as a note's time is written. */
	setTime(time: string): this {
		this.#model.time = time;
		return this;
	}

	/** Above the time, for a timeline that spans days; none to take it away. */
	setDate(date?: string): this {
		this.#model.date = date;
		return this;
	}

	/** An icon in a box on the rail, as a page has, in place of the dot; none for the dot. */
	setIcon(icon?: string): this {
		this.#model.icon = icon;
		return this;
	}

	/** Make the date and time a button, such as to the note on its day. */
	onClickTime(click: () => unknown, title?: string): this {
		this.#model.click = click;
		this.#model.clickTitle = title;
		return this;
	}

	/** Take the item off the timeline. What the plugin drew in it is its to destroy first. */
	remove() {
		if (!this.#drawn) return;
		void unmount(this.#drawn);
		this.#drawn = null;
		this.itemEl.remove();
		this.#onremove();
	}
}
