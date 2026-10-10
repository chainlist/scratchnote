import {
	ItemView,
	Plugin,
	PluginSettingTab,
	Setting,
	Timeline,
	type App,
	type Note,
	type NoteLabel,
	type RenderedMarkdown
} from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';
import { element } from '../dom';
import { named } from './names';

/** Lucide's tag. */
const TAG =
	'<path d="M12.586 2.586A2 2 0 0 0 11.172 2H4a2 2 0 0 0-2 2v7.172a2 2 0 0 0 .586 1.414l8.704 8.704a2.426 2.426 0 0 0 3.42 0l6.58-6.58a2.426 2.426 0 0 0 0-3.42z"/><circle cx="7.5" cy="7.5" r=".5" fill="currentColor"/>';
/** Lucide's file-text, beside a page in a label's notes. */
const PAGE_ICON =
	'<path d="M15 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V7Z"/><path d="M14 2v4a2 2 0 0 0 2 2h4"/><path d="M10 9H8"/><path d="M16 13H8"/><path d="M16 17H8"/>';
/** The page, at `/plugin/labels/`, with `?label=` for one label's notes. */
const PAGE = 'labels';
/**
 * The shares a label can be asked to reach before it shows, as the settings
 * offer them. Below, it is a guess. On labelled test notes, a part of life
 * read at 50% or more was right 96% of the time.
 */
const SHARES = ['0.3', '0.4', '0.5', '0.6', '0.7', '0.8'];

interface LabelsSettings {
	/** The share a label needs to show, one of `SHARES`. */
	sure: string;
}

const DEFAULTS: LabelsSettings = { sure: '0.5' };
/** Notes a label's page draws at once; Show more draws as many again. */
const SHOWN = 100;

/**
 * What a note is shown as: its part of life when the app is sure of it,
 * with its job family when sure of that too. `key`, such as
 * `work/software`, names it in the page's query.
 */
interface Shown {
	life: string;
	job: string | null;
	key: string;
	text: string;
}

function shown(label: NoteLabel | undefined, sure: number): Shown | null {
	if (!label || label.life.score < sure) return null;
	const life = label.life.label;
	const job = label.job && label.job.score >= sure ? label.job.label : null;
	return job
		? { life, job, key: `${life}/${job}`, text: `${named(life)} · ${named(job)}` }
		: { life, job, key: life, text: named(life) };
}

/**
 * Labels, a core plugin (SPEC 3.14): what each note is about, read from its
 * meaning, on its card and on a page listing the notes by label.
 */
export class LabelsPlugin extends Plugin {
	/** Every embedded note's label, by id, as the app last read them. */
	labels = $state.raw<Record<string, NoteLabel>>({});
	/** State, so the cards redraw their chips when it changes. */
	settings = $state<LabelsSettings>({ ...DEFAULTS });
	/** The page, while it is open. */
	page: LabelsPage | null = null;

	/** The share a label needs to show. */
	get sure() {
		return Number(this.settings.sure);
	}

	async onload() {
		// Only the main window draws note cards.
		if (this.app.window === 'capture') return;
		await this.loadSettings();
		this.addSettingTab(new LabelsSettingTab(this.app, this));
		const open = (label?: string) =>
			this.app.workspace.openPage(PAGE, label ? { label } : undefined);
		this.registerPage(PAGE, () => new LabelsPage(this));
		this.addRibbonIcon(TAG, m.labels_plugin_name, () => open());
		this.addCommand({ id: 'show', name: m.labels_show, icon: TAG, callback: () => open() });
		this.registerNoteChip((note) => {
			const label = shown(this.labels[note.id], this.sure);
			return (
				label && {
					text: label.text,
					icon: TAG,
					title: m.labels_open({ label: label.text }),
					onClick: () => open(label.key)
				}
			);
		});
		const reload = () => void this.reload();
		// Embedding labels the notes saved; another space opening brings its own.
		this.registerEvent(this.app.on('meaning-changed', reload));
		this.registerEvent(this.app.on('notes-changed', reload));
		await this.reload();
	}

	async onExternalSettingsChange() {
		await this.loadSettings();
		await this.page?.draw();
	}

	async loadSettings() {
		const saved = (await this.loadData()) as Partial<LabelsSettings> | null;
		this.settings = { ...DEFAULTS, ...saved };
		if (!SHARES.includes(this.settings.sure)) this.settings.sure = DEFAULTS.sure;
	}

	async reload() {
		try {
			this.labels = await this.app.notes.labels();
		} catch (e) {
			console.error('labels: could not read the labels', e);
		}
		await this.page?.draw();
	}
}

/** Every label with how many notes have it, or one label's notes, newest first. */
class LabelsPage extends ItemView {
	#plugin: LabelsPlugin;
	#drawn: RenderedMarkdown[] = [];
	#count = 0;
	/** How many of the label's notes show, kept as the page redraws. */
	#limit = SHOWN;

	constructor(plugin: LabelsPlugin) {
		super();
		this.#plugin = plugin;
	}

	getDisplayText() {
		return this.params.label ? this.#name(this.params.label) : m.labels_all();
	}

	getIcon() {
		return TAG;
	}

	getDetail() {
		return this.params.label ? m.tags_notes({ count: this.#count }) : undefined;
	}

	async onOpen() {
		this.#plugin.page = this;
		await this.draw();
	}

	onClose() {
		this.#plugin.page = null;
		this.#clear();
	}

	#name(key: string) {
		return key.split('/').map(named).join(' · ');
	}

	#clear() {
		for (const drawn of this.#drawn.splice(0)) drawn.destroy();
		this.containerEl.replaceChildren();
	}

	async draw() {
		const shownById = Object.entries(this.#plugin.labels).map(
			([id, label]) => [id, shown(label, this.#plugin.sure)] as const
		);
		if (this.params.label) await this.#drawLabel(this.params.label, shownById);
		else this.#drawAll(shownById);
	}

	#drawAll(labelled: (readonly [string, Shown | null])[]) {
		this.#clear();
		if (labelled.length === 0) {
			this.containerEl.append(element('p', 'labels-empty', m.labels_none()));
			return;
		}
		const lives: Record<string, number> = {};
		const jobs: Record<string, number> = {};
		let unsure = 0;
		for (const [, label] of labelled) {
			if (!label) {
				unsure++;
				continue;
			}
			lives[label.life] = (lives[label.life] ?? 0) + 1;
			if (label.job) jobs[label.job] = (jobs[label.job] ?? 0) + 1;
		}
		const byCount = (counts: Record<string, number>) =>
			Object.entries(counts).sort((a, b) => b[1] - a[1] || named(a[0]).localeCompare(named(b[0])));
		const row = (key: string, count: number, nested = false) => {
			const button = element('button', nested ? 'labels-row labels-nested' : 'labels-row');
			button.type = 'button';
			// Under Work, a job family needs no "Work" before it.
			button.append(
				element('span', 'labels-name', nested ? named(key.split('/')[1]) : this.#name(key)),
				element('span', 'labels-count', String(count))
			);
			button.addEventListener('click', () => this.app.workspace.openPage(PAGE, { label: key }));
			return button;
		};
		const list = element('div', 'labels-list');
		for (const [life, count] of byCount(lives)) {
			list.append(row(life, count));
			if (life === 'work')
				for (const [job, n] of byCount(jobs)) list.append(row(`work/${job}`, n, true));
		}
		this.containerEl.append(list);
		if (unsure > 0)
			this.containerEl.append(element('p', 'labels-unsure', m.labels_unsure({ count: unsure })));
	}

	async #drawLabel(key: string, labelled: (readonly [string, Shown | null])[]) {
		const [life, job] = key.split('/');
		const ids = labelled
			.filter(([, label]) => label && label.life === life && (!job || label.job === job))
			.map(([id]) => id);
		const notes = (await this.app.notes.get(ids)).sort(
			(a: Note, b: Note) => b.date.localeCompare(a.date) || b.time.localeCompare(a.time)
		);
		this.#count = notes.length;
		this.refreshHeader();
		this.#clear();

		const back = element('button', 'labels-back', m.labels_all());
		back.type = 'button';
		back.addEventListener('click', () => this.app.workspace.openPage(PAGE));
		this.containerEl.append(back);

		// The year only when it is not this one, so the date fits over the time.
		const thisYear = String(new Date().getFullYear());
		const short = new Intl.DateTimeFormat(this.app.locale, { day: 'numeric', month: 'short' });
		const long = new Intl.DateTimeFormat(this.app.locale, {
			day: 'numeric',
			month: 'short',
			year: 'numeric'
		});
		const dated = (date: string) =>
			(date.startsWith(thisYear) ? short : long).format(new Date(`${date}T00:00`));

		const timeline = new Timeline(this.containerEl);
		for (const note of notes.slice(0, this.#limit)) {
			const item = timeline.addItem((item) =>
				item
					.setDate(dated(note.date))
					.setTime(note.time)
					.setIcon(note.kind === 'page' ? PAGE_ICON : undefined)
					.onClickTime(() => this.app.workspace.openNote(note))
			);
			if (note.subject) item.contentEl.append(element('p', 'labels-title', note.subject));
			const body = element('div');
			item.contentEl.append(body);
			this.#drawn.push(this.app.markdown.render(body, note.body));
		}
		if (notes.length > this.#limit) {
			const more = element('button', 'labels-more', m.search_show_more());
			more.type = 'button';
			more.addEventListener('click', () => {
				this.#limit += SHOWN;
				void this.draw();
			});
			this.containerEl.append(more);
		}
	}
}

class LabelsSettingTab extends PluginSettingTab {
	#plugin: LabelsPlugin;

	constructor(app: App, plugin: LabelsPlugin) {
		super(app, plugin);
		this.#plugin = plugin;
		this.icon = TAG;
	}

	display() {
		const plugin = this.#plugin;
		const percent = new Intl.NumberFormat(this.app.locale, { style: 'percent' });
		new Setting(this.containerEl)
			.setName(m.labels_setting_sure())
			.setDesc(m.labels_setting_sure_hint())
			.addDropdown((dropdown) => {
				for (const share of SHARES) dropdown.addOption(share, percent.format(Number(share)));
				dropdown.setValue(plugin.settings.sure).onChange(async (share) => {
					plugin.settings.sure = share;
					await plugin.saveData($state.snapshot(plugin.settings));
					await plugin.page?.draw();
				});
			});
	}
}
