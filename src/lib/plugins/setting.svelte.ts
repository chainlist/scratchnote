import { mount, unmount } from 'svelte';
import SettingItem from './SettingItem.svelte';

/**
 * A row of a plugin's settings tab, as Obsidian's `Setting`: a name, a
 * line under it, and controls on the right. It draws with the app's own
 * switches, fields and buttons, so a plugin's tab reads as the app's tabs:
 * rows in a box, and headings between boxes, or `SettingSection`s.
 *
 * ```js
 * new Setting(containerEl)
 *   .setName('Colour')
 *   .setDesc('How highlighted text is painted.')
 *   .addDropdown((d) => d.addOption('yellow', 'Yellow').setValue(v).onChange(save));
 * ```
 */

export type Control =
	| { kind: 'toggle'; value: boolean; disabled: boolean; change?: (value: boolean) => unknown }
	| {
			kind: 'text';
			value: string;
			placeholder: string;
			disabled: boolean;
			change?: (value: string) => unknown;
	  }
	| {
			kind: 'dropdown';
			value: string;
			options: { value: string; label: string }[];
			disabled: boolean;
			change?: (value: string) => unknown;
	  }
	| {
			kind: 'button';
			text: string;
			variant: 'secondary' | 'default' | 'destructive';
			disabled: boolean;
			click?: () => unknown;
	  };

export interface SettingModel {
	name: string;
	desc: string;
	heading: boolean;
	controls: Control[];
}

/** Rows of one box: the first and last round it off, a heading ends it. */
const ROW =
	'flex items-center justify-between gap-6 border-x border-b bg-card px-4 py-3 first:rounded-t-lg first:border-t last:rounded-b-lg [.setting-heading+&]:rounded-t-lg [.setting-heading+&]:border-t [&:has(+.setting-heading)]:rounded-b-lg';
const HEADING = 'setting-heading mt-6 mb-2 flex items-center justify-between gap-6 first:mt-0';
const SECTION = 'setting-section mt-6 first:mt-0';

/** What each container holds, to unmount before it is drawn again. */
const drawn = new WeakMap<HTMLElement, (() => void)[]>();

function track(el: HTMLElement, remove: () => void) {
	const list = drawn.get(el) ?? [];
	list.push(remove);
	drawn.set(el, list);
}

/** Take away the settings drawn in `el`, and empty it. */
export function clearSettings(el: HTMLElement) {
	for (const remove of drawn.get(el) ?? []) remove();
	drawn.delete(el);
	el.replaceChildren();
}

/** Run a plugin's handler, which may be async, and say when it fails. */
export function call<T>(handler: ((value: T) => unknown) | undefined, value: T) {
	try {
		void Promise.resolve(handler?.(value)).catch((e) => console.error(e));
	} catch (e) {
		console.error(e);
	}
}

export class Setting {
	readonly settingEl: HTMLElement;
	#model: SettingModel = $state({ name: '', desc: '', heading: false, controls: [] });

	constructor(containerEl: HTMLElement) {
		this.settingEl = document.createElement('div');
		this.settingEl.className = ROW;
		containerEl.append(this.settingEl);
		const item = mount(SettingItem, { target: this.settingEl, props: { model: this.#model } });
		track(containerEl, () => void unmount(item));
	}

	setName(name: string): this {
		this.#model.name = name;
		return this;
	}

	setDesc(desc: string): this {
		this.#model.desc = desc;
		return this;
	}

	/**
	 * A title over the rows that follow, which start a box of their own. It
	 * takes a description and controls as a row does.
	 */
	setHeading(): this {
		this.#model.heading = true;
		this.settingEl.className = HEADING;
		return this;
	}

	/** The control just added, as the state the row draws from. */
	#add<C extends Control>(control: C): C {
		this.#model.controls.push(control);
		return this.#model.controls[this.#model.controls.length - 1] as C;
	}

	addToggle(build: (toggle: ToggleComponent) => unknown): this {
		build(new ToggleComponent(this.#add({ kind: 'toggle', value: false, disabled: false })));
		return this;
	}

	addText(build: (text: TextComponent) => unknown): this {
		build(
			new TextComponent(this.#add({ kind: 'text', value: '', placeholder: '', disabled: false }))
		);
		return this;
	}

	addDropdown(build: (dropdown: DropdownComponent) => unknown): this {
		build(
			new DropdownComponent(
				this.#add({ kind: 'dropdown', value: '', options: [], disabled: false })
			)
		);
		return this;
	}

	addButton(build: (button: ButtonComponent) => unknown): this {
		build(
			new ButtonComponent(
				this.#add({ kind: 'button', text: '', variant: 'secondary', disabled: false })
			)
		);
		return this;
	}
}

/**
 * A titled box of settings, as the app's own tabs are laid out, for a tab
 * that holds several things. With a switch, as a feature that is switched
 * on and off has, its settings show only while it is on.
 *
 * ```js
 * const tasks = new SettingSection(containerEl)
 *   .setName('Tasks')
 *   .setDesc('Boxes to tick in notes and pages.')
 *   .addToggle((t) => t.setValue(on).onChange(save));
 * new Setting(tasks.contentEl).setName('Order').addDropdown(...);
 * ```
 */
export class SettingSection {
	readonly sectionEl: HTMLElement;
	/** Where its settings go. */
	readonly contentEl: HTMLElement;
	#heading: Setting;

	constructor(containerEl: HTMLElement) {
		this.sectionEl = document.createElement('div');
		this.sectionEl.className = SECTION;
		this.contentEl = document.createElement('div');
		containerEl.append(this.sectionEl);
		this.#heading = new Setting(this.sectionEl).setHeading();
		this.sectionEl.append(this.contentEl);
		track(containerEl, () => {
			clearSettings(this.contentEl);
			clearSettings(this.sectionEl);
		});
	}

	setName(name: string): this {
		this.#heading.setName(name);
		return this;
	}

	setDesc(desc: string): this {
		this.#heading.setDesc(desc);
		return this;
	}

	/** A switch by the title: the settings show while it is on. */
	addToggle(build: (toggle: ToggleComponent) => unknown): this {
		this.#heading.addToggle((toggle) => {
			build(toggle);
			const stop = $effect.root(() => {
				$effect(() => {
					this.contentEl.hidden = !toggle.getValue();
				});
			});
			track(this.sectionEl, stop);
		});
		return this;
	}
}

type Of<K extends Control['kind']> = Extract<Control, { kind: K }>;

export class ToggleComponent {
	#control: Of<'toggle'>;
	constructor(control: Of<'toggle'>) {
		this.#control = control;
	}
	getValue() {
		return this.#control.value;
	}
	setValue(value: boolean): this {
		this.#control.value = value;
		return this;
	}
	setDisabled(disabled: boolean): this {
		this.#control.disabled = disabled;
		return this;
	}
	onChange(change: (value: boolean) => unknown): this {
		this.#control.change = change;
		return this;
	}
}

export class TextComponent {
	#control: Of<'text'>;
	constructor(control: Of<'text'>) {
		this.#control = control;
	}
	getValue() {
		return this.#control.value;
	}
	setValue(value: string): this {
		this.#control.value = value;
		return this;
	}
	setPlaceholder(placeholder: string): this {
		this.#control.placeholder = placeholder;
		return this;
	}
	setDisabled(disabled: boolean): this {
		this.#control.disabled = disabled;
		return this;
	}
	/** Called as the text is typed. */
	onChange(change: (value: string) => unknown): this {
		this.#control.change = change;
		return this;
	}
}

export class DropdownComponent {
	#control: Of<'dropdown'>;
	constructor(control: Of<'dropdown'>) {
		this.#control = control;
	}
	addOption(value: string, label: string): this {
		this.#control.options.push({ value, label });
		return this;
	}
	addOptions(options: Record<string, string>): this {
		for (const [value, label] of Object.entries(options)) this.addOption(value, label);
		return this;
	}
	getValue() {
		return this.#control.value;
	}
	setValue(value: string): this {
		this.#control.value = value;
		return this;
	}
	setDisabled(disabled: boolean): this {
		this.#control.disabled = disabled;
		return this;
	}
	onChange(change: (value: string) => unknown): this {
		this.#control.change = change;
		return this;
	}
}

export class ButtonComponent {
	#control: Of<'button'>;
	constructor(control: Of<'button'>) {
		this.#control = control;
	}
	setButtonText(text: string): this {
		this.#control.text = text;
		return this;
	}
	/** The call to action: drawn in the accent. */
	setCta(): this {
		this.#control.variant = 'default';
		return this;
	}
	/** For what cannot be undone. */
	setWarning(): this {
		this.#control.variant = 'destructive';
		return this;
	}
	setDisabled(disabled: boolean): this {
		this.#control.disabled = disabled;
		return this;
	}
	onClick(click: () => unknown): this {
		this.#control.click = click;
		return this;
	}
}
