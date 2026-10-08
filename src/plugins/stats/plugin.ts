import { ItemView, Plugin, PluginSettingTab, Setting, type App } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import { getLocale } from '#lib/paraglide/runtime.js';

/** Lucide's chart-column. */
const CHART =
	'<path d="M3 3v16a2 2 0 0 0 2 2h16"/><path d="M18 17V9"/><path d="M13 17V5"/><path d="M8 17v-3"/>';
/** The panel, docked beside the view. */
const VIEW = 'stats';
/** How far back the heatmap can go. */
const WEEKS = ['12', '20', '26'];

interface StatsSettings {
	/** How many weeks the heatmap shows, one of `WEEKS`. */
	weeks: string;
}

const DEFAULTS: StatsSettings = { weeks: '20' };

/** The day `days` after `date`, by the calendar, so a clock change never skips one. */
function after(date: Date, days: number): Date {
	const day = new Date(date);
	day.setDate(day.getDate() + days);
	return day;
}

/** A day as the app names it, `2026-09-29`, in the local timezone. */
function iso(date: Date): string {
	const pad = (n: number) => String(n).padStart(2, '0');
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function element<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	className?: string,
	text?: string
): HTMLElementTagNameMap[K] {
	const el = document.createElement(tag);
	if (className) el.className = className;
	if (text !== undefined) el.textContent = text;
	return el;
}

/**
 * Stats, a core plugin (SPEC 3.11): how much the space holds, the writing
 * streak, and a heatmap of the days written, in a panel beside the day.
 */
export class StatsPlugin extends Plugin {
	settings: StatsSettings = { ...DEFAULTS };
	/** The panel, while it is open. */
	view: StatsView | null = null;

	async onload() {
		await this.loadSettings();
		this.registerView(VIEW, () => new StatsView(this));
		this.addRibbonIcon(CHART, m.stats_plugin_name, () => this.app.workspace.openView(VIEW));
		this.addCommand({
			id: 'show',
			name: m.stats_show,
			icon: CHART,
			callback: () => this.app.workspace.openView(VIEW)
		});
		this.addSettingTab(new StatsSettingTab(this.app, this));
	}

	onunload() {
		this.app.workspace.closeView(VIEW);
	}

	async onExternalSettingsChange() {
		await this.loadSettings();
		await this.view?.draw();
	}

	async loadSettings() {
		const saved = (await this.loadData()) as Partial<StatsSettings> | null;
		this.settings = { ...DEFAULTS, ...saved };
	}
}

/** How much the space holds, and a heatmap of the days written, in the dock. */
class StatsView extends ItemView {
	#plugin: StatsPlugin;
	#off: (() => void) | null = null;

	constructor(plugin: StatsPlugin) {
		super();
		this.#plugin = plugin;
	}

	getDisplayText() {
		return m.stats_plugin_name();
	}

	getIcon() {
		return CHART;
	}

	async onOpen() {
		this.#plugin.view = this;
		this.#off = this.app.on('notes-changed', () => void this.draw());
		await this.draw();
	}

	onClose() {
		this.#plugin.view = null;
		this.#off?.();
	}

	async draw() {
		const [days, pages] = await Promise.all([this.app.notes.days(), this.app.notes.pages()]);
		const counts = new Map(days.map((day) => [day.date, day.count]));
		const total = days.reduce((sum, day) => sum + day.count, 0);
		// Pages are left out of the words, so they are left out of the notes too.
		const notes = total - pages.length;
		const words = days.reduce((sum, day) => sum + day.words, 0);
		const perNote = notes > 0 ? Math.round(words / notes) : 0;

		// Days written in a row, up to today, or up to yesterday while today is empty.
		const today = new Date();
		let streak = 0;
		for (let day = today; ; day = after(day, -1)) {
			if (counts.has(iso(day))) streak++;
			else if (iso(day) !== iso(today)) break;
		}

		const figures = element('div', 'stats-figures');
		for (const [count, label] of [
			[total, m.stats_notes],
			[pages.length, m.stats_pages],
			[days.length, m.stats_days],
			[streak, m.stats_streak],
			[perNote, m.stats_words]
		] as const) {
			const figure = element('div', 'stats-figure');
			figure.append(
				element('span', 'stats-value', String(count)),
				element('span', '', label({ count }))
			);
			figures.append(figure);
		}

		// Week columns, Monday on top, ending with this week. Each day is named
		// in the interface language with its count, read out and on hover.
		const weeks = Number(this.#plugin.settings.weeks);
		const busiest = Math.max(1, ...counts.values());
		const monday = after(today, -((today.getDay() + 6) % 7));
		const named = new Intl.DateTimeFormat(getLocale(), {
			weekday: 'short',
			day: 'numeric',
			month: 'short'
		});
		const heading = element('h3', 'stats-heading', m.stats_last_weeks({ weeks }));
		heading.id = 'stats-heading';
		const grid = element('div', 'stats-heatmap');
		grid.style.setProperty('--weeks', String(weeks));
		grid.setAttribute('role', 'group');
		grid.setAttribute('aria-labelledby', heading.id);
		const cells: HTMLButtonElement[] = [];
		for (let week = weeks - 1; week >= 0; week--) {
			for (let weekday = 0; weekday < 7; weekday++) {
				const day = after(monday, weekday - week * 7);
				const date = iso(day);
				const count = counts.get(date) ?? 0;
				const cell = element('button', 'stats-day');
				cell.type = 'button';
				cell.title = `${named.format(day)}: ${m.tags_notes({ count })}`;
				cell.tabIndex = -1;
				cell.style.setProperty('--level', String(count / busiest));
				if (count === 0) cell.classList.add('stats-none');
				if (day > today) cell.disabled = true;
				else cell.addEventListener('click', () => this.app.workspace.openDay(date));
				cells.push(cell);
				grid.append(cell);
			}
		}

		// One stop for Tab, on today: the arrows go a day up or down and a
		// week left or right, Home and End to the first day and today.
		const last = cells.findLastIndex((cell) => !cell.disabled);
		cells[last].tabIndex = 0;
		const steps: Record<string, number> = {
			ArrowUp: -1,
			ArrowDown: 1,
			ArrowLeft: -7,
			ArrowRight: 7
		};
		grid.addEventListener('keydown', (event) => {
			const at = cells.indexOf(event.target as HTMLButtonElement);
			if (at < 0) return;
			const to =
				event.key === 'Home'
					? 0
					: event.key === 'End'
						? last
						: event.key in steps
							? Math.min(last, Math.max(0, at + steps[event.key]))
							: null;
			if (to === null) return;
			event.preventDefault();
			cells[at].tabIndex = -1;
			cells[to].tabIndex = 0;
			cells[to].focus();
		});

		this.containerEl.replaceChildren(figures, heading, grid);
	}
}

class StatsSettingTab extends PluginSettingTab {
	#plugin: StatsPlugin;

	constructor(app: App, plugin: StatsPlugin) {
		super(app, plugin);
		this.#plugin = plugin;
		this.icon = CHART;
	}

	display() {
		const plugin = this.#plugin;
		new Setting(this.containerEl)
			.setName(m.stats_setting_weeks())
			.setDesc(m.stats_setting_weeks_hint())
			.addDropdown((dropdown) => {
				for (const weeks of WEEKS) dropdown.addOption(weeks, m.stats_weeks({ weeks }));
				dropdown.setValue(plugin.settings.weeks).onChange(async (weeks) => {
					plugin.settings.weeks = weeks;
					await plugin.saveData(plugin.settings);
					await plugin.view?.draw();
				});
			});
	}
}
