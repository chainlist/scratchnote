/* eslint-disable @typescript-eslint/no-require-imports -- plugins are CommonJS, as Obsidian's */
const { ItemView, Plugin, PluginSettingTab, Setting } = require('scratchnote');

/** Lucide's chart-column. */
const CHART =
	'<path d="M3 3v16a2 2 0 0 0 2 2h16"/><path d="M18 17V9"/><path d="M13 17V5"/><path d="M8 17v-3"/>';
const VIEW = 'space-stats';
const DEFAULTS = { weeks: '20' };

/** The day `days` after `date`, by the calendar, so a clock change never skips one. */
function after(date, days) {
	const day = new Date(date);
	day.setDate(day.getDate() + days);
	return day;
}

/** A day as the app names it, `2026-09-29`, in the local timezone. */
function iso(date) {
	const pad = (n) => String(n).padStart(2, '0');
	return `${date.getFullYear()}-${pad(date.getMonth() + 1)}-${pad(date.getDate())}`;
}

function element(tag, className, text) {
	const el = document.createElement(tag);
	if (className) el.className = className;
	if (text !== undefined) el.textContent = text;
	return el;
}

class SpaceStatsPlugin extends Plugin {
	settings = { ...DEFAULTS };
	/** The panel, while it is open. */
	view = null;

	async onload() {
		this.settings = { ...DEFAULTS, ...(await this.loadData()) };
		this.registerView(VIEW, () => new StatsView(this));
		this.addRibbonIcon(CHART, 'Space stats', () => this.app.workspace.openView(VIEW));
		this.addCommand({
			id: 'show',
			name: 'Show space stats',
			icon: CHART,
			callback: () => this.app.workspace.openView(VIEW)
		});
		this.addSettingTab(new StatsSettingTab(this.app, this));
	}

	onunload() {
		this.app.workspace.closeView(VIEW);
	}

	async onExternalSettingsChange() {
		this.settings = { ...DEFAULTS, ...(await this.loadData()) };
		await this.view?.draw();
	}
}

/** How much the space holds, and a heatmap of the days written, in the dock. */
class StatsView extends ItemView {
	constructor(plugin) {
		super();
		this.plugin = plugin;
	}

	getDisplayText() {
		return 'Space stats';
	}

	getIcon() {
		return CHART;
	}

	async onOpen() {
		this.plugin.view = this;
		this.off = this.app.on('notes-changed', () => void this.draw());
		await this.draw();
	}

	onClose() {
		this.plugin.view = null;
		this.off?.();
	}

	async draw() {
		const [days, pages] = await Promise.all([this.app.notes.days(), this.app.notes.pages()]);
		const counts = new Map(days.map((day) => [day.date, day.count]));
		const total = days.reduce((sum, day) => sum + day.count, 0);

		// Days written in a row, up to today, or up to yesterday while today is empty.
		const today = new Date();
		let streak = 0;
		for (let day = today; ; day = after(day, -1)) {
			if (counts.has(iso(day))) streak++;
			else if (iso(day) !== iso(today)) break;
		}

		const figures = element('div', 'stats-figures');
		for (const [value, label] of [
			[total, 'notes and pages'],
			[pages.length, 'pages'],
			[days.length, 'days written'],
			[streak, streak === 1 ? 'day in a row' : 'days in a row']
		]) {
			const figure = element('div', 'stats-figure');
			figure.append(element('span', 'stats-value', String(value)), element('span', '', label));
			figures.append(figure);
		}

		// Week columns, Monday on top, ending with this week.
		const weeks = Number(this.plugin.settings.weeks);
		const busiest = Math.max(1, ...counts.values());
		const monday = after(today, -((today.getDay() + 6) % 7));
		const grid = element('div', 'stats-heatmap');
		grid.style.setProperty('--weeks', String(weeks));
		for (let week = weeks - 1; week >= 0; week--) {
			for (let weekday = 0; weekday < 7; weekday++) {
				const day = after(monday, weekday - week * 7);
				const date = iso(day);
				const count = counts.get(date) ?? 0;
				const cell = element('button', 'stats-day');
				cell.type = 'button';
				cell.title = `${date}: ${count}`;
				cell.style.setProperty('--level', String(count / busiest));
				if (count === 0) cell.classList.add('stats-none');
				if (day > today) cell.disabled = true;
				else cell.addEventListener('click', () => this.app.workspace.openDay(date));
				grid.append(cell);
			}
		}

		this.containerEl.replaceChildren(
			figures,
			element('h3', 'stats-heading', `The last ${weeks} weeks`),
			grid
		);
	}
}

class StatsSettingTab extends PluginSettingTab {
	constructor(app, plugin) {
		super(app, plugin);
		this.icon = CHART;
	}

	display() {
		const { plugin } = this;
		new Setting(this.containerEl)
			.setName('Weeks shown')
			.setDesc('How far back the heatmap goes.')
			.addDropdown((dropdown) =>
				dropdown
					.addOptions({ 12: '12 weeks', 20: '20 weeks', 26: '26 weeks' })
					.setValue(plugin.settings.weeks)
					.onChange(async (weeks) => {
						plugin.settings.weeks = weeks;
						await plugin.saveData(plugin.settings);
						await plugin.view?.draw();
					})
			);
	}
}

module.exports = SpaceStatsPlugin;
