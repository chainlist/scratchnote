import {
	Plugin,
	PluginSettingTab,
	Setting,
	SettingSection,
	type App,
	type Component
} from '$lib/plugins/api';
import { m } from '$lib/paraglide/messages';
import { COLORS, Highlights, type HighlightColor } from './highlights/feature';
import { Tasks } from './tasks/feature';

/** Lucide's shapes. */
const SHAPES =
	'<path d="M8.3 10a.7.7 0 0 1-.626-1.079L11.4 3a.7.7 0 0 1 1.198-.043L16.3 8.9a.7.7 0 0 1-.572 1.1Z"/><rect x="3" y="14" width="7" height="7" rx="1"/><circle cx="17.5" cy="17.5" r="3.5"/>';

interface BasicsSettings {
	tasks: {
		on: boolean;
		/** The button on the left edge that opens the open tasks. */
		ribbon: boolean;
		/** Which tasks come first. */
		order: 'newest' | 'oldest';
	};
	highlights: {
		on: boolean;
		color: HighlightColor;
	};
}

const DEFAULTS: BasicsSettings = {
	tasks: { on: true, ribbon: true, order: 'newest' },
	highlights: { on: true, color: 'yellow' }
};

/**
 * Basics, the core plugin (SPEC 3.8): Tasks and Highlights, each a part the
 * plugin's settings switch on and off by itself.
 */
export class BasicsPlugin extends Plugin {
	settings = $state<BasicsSettings>(structuredClone(DEFAULTS));
	#tasks: Tasks | null = null;
	#highlights: Highlights | null = null;

	async onload() {
		// The first card waits a moment for this, so it is drawn with the parts on.
		await this.loadSettings();
		this.addSettingTab(new BasicsSettingTab(this.app, this));
	}

	onExternalSettingsChange() {
		void this.loadSettings();
	}

	async loadSettings() {
		const saved = (await this.loadData()) as Partial<BasicsSettings> | null;
		this.settings = {
			tasks: { ...DEFAULTS.tasks, ...saved?.tasks },
			highlights: { ...DEFAULTS.highlights, ...saved?.highlights }
		};
		this.#apply();
	}

	async saveSettings() {
		await this.saveData($state.snapshot(this.settings));
		this.#apply();
	}

	/** Each part loaded while it is on, and set as its settings say. */
	#apply() {
		const { tasks, highlights } = this.settings;
		this.#tasks = this.#part(this.#tasks, tasks.on, () => new Tasks(this));
		this.#highlights = this.#part(this.#highlights, highlights.on, () => new Highlights(this));
		this.#tasks?.update();
		this.#highlights?.update();
	}

	#part<T extends Component>(current: T | null, on: boolean, create: () => T): T | null {
		if (on) return current ?? this.addChild(create());
		if (current) this.removeChild(current);
		return null;
	}
}

/** A section per part, with the part's switch and its settings. */
class BasicsSettingTab extends PluginSettingTab {
	#plugin: BasicsPlugin;

	constructor(app: App, plugin: BasicsPlugin) {
		super(app, plugin);
		this.#plugin = plugin;
		this.icon = SHAPES;
	}

	display() {
		const plugin = this.#plugin;
		const save = () => plugin.saveSettings();

		const tasks = new SettingSection(this.containerEl)
			.setName(m.tasks_name())
			.setDesc(m.tasks_description())
			.addToggle((toggle) =>
				toggle.setValue(plugin.settings.tasks.on).onChange((on) => {
					plugin.settings.tasks.on = on;
					return save();
				})
			);
		new Setting(tasks.contentEl)
			.setName(m.tasks_setting_ribbon())
			.setDesc(m.tasks_setting_ribbon_hint())
			.addToggle((toggle) =>
				toggle.setValue(plugin.settings.tasks.ribbon).onChange((ribbon) => {
					plugin.settings.tasks.ribbon = ribbon;
					return save();
				})
			);
		new Setting(tasks.contentEl)
			.setName(m.tasks_setting_order())
			.setDesc(m.tasks_setting_order_hint())
			.addDropdown((dropdown) =>
				dropdown
					.addOption('newest', m.tasks_order_newest())
					.addOption('oldest', m.tasks_order_oldest())
					.setValue(plugin.settings.tasks.order)
					.onChange((order) => {
						plugin.settings.tasks.order = order === 'oldest' ? 'oldest' : 'newest';
						return save();
					})
			);

		const highlights = new SettingSection(this.containerEl)
			.setName(m.highlights_name())
			.setDesc(m.highlights_description())
			.addToggle((toggle) =>
				toggle.setValue(plugin.settings.highlights.on).onChange((on) => {
					plugin.settings.highlights.on = on;
					return save();
				})
			);
		new Setting(highlights.contentEl)
			.setName(m.highlights_setting_color())
			.setDesc(m.highlights_setting_color_hint())
			.addDropdown((dropdown) => {
				for (const [color, name] of Object.entries(COLORS)) dropdown.addOption(color, name());
				dropdown.setValue(plugin.settings.highlights.color).onChange((color) => {
					if (color in COLORS) plugin.settings.highlights.color = color as HighlightColor;
					return save();
				});
			});
	}
}
