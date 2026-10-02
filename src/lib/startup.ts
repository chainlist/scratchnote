import { app } from '#lib/plugins/app.js';
import { pluginName, pluginStartup, type StartupStep } from '#lib/plugins/loader.js';

/**
 * How long this window took to start: loading the app's code, the wait on
 * the plugins with each plugin's load, and the first view. Times run from
 * when the window began loading. The root layout records them once the
 * first view is up; the console shows them, and so does Settings > General.
 */

/** A phase of the startup, or a step of the wait on the plugins. */
export interface StartupRow {
	step: 'app' | 'plugins' | 'view' | StartupStep['step'];
	ms: number;
	/** A step of the wait on the plugins, shown under it. */
	nested: boolean;
	plugin?: StartupStep['plugin'];
}

export interface StartupTimes {
	/** From when the window began loading until its first view was up. */
	total: number;
	/** How much of it the first view waited on the plugins. */
	plugins: number;
	rows: StartupRow[];
}

let times: StartupTimes | null = null;

/** This window's startup times, once its first view is up; null before. */
export const startupTimes = () => times;

/** Keep the startup times, now that the first view is up, and log them. */
export function recordStartup() {
	const total = performance.now();
	const { start, end, steps } = pluginStartup;
	times = {
		total,
		plugins: end - start,
		rows: [
			{ step: 'app', ms: start, nested: false },
			{ step: 'plugins', ms: end - start, nested: false },
			...steps.map((step) => ({ ...step, nested: true })),
			{ step: 'view', ms: total - end, nested: false }
		]
	};
	log(times);
}

/** The widest bar, for a step that took the whole startup. */
const BAR = 32;

const STYLE = {
	badge: 'background:#7c3aed;color:#fff;border-radius:3px;padding:1px 6px;font-weight:600',
	total: 'font-weight:700',
	step: '',
	nested: 'color:#a3a3a3',
	time: 'font-weight:600',
	bar: 'color:#8b5cf6',
	nestedBar: 'color:#c4b5fd',
	note: 'color:#8a8a8a'
};

/** The console's names, in English as the rest of the console is. */
const NAMES: Record<Exclude<StartupRow['step'], 'plugin'>, string> = {
	app: 'Loading the app',
	plugins: 'Waiting on plugins',
	list: 'Plugin list',
	listeners: 'Change listeners',
	view: 'First view'
};

/** A duration as the startup times show it: tenths under 10 ms, whole ms above. */
export const duration = (ms: number, locale = 'en') =>
	new Intl.NumberFormat(locale, {
		style: 'unit',
		unit: 'millisecond',
		maximumFractionDigits: ms < 10 ? 1 : 0
	}).format(ms);

function log({ total, plugins, rows }: StartupTimes) {
	const name = (row: StartupRow) =>
		row.step === 'plugin' ? pluginName(row.plugin!.id) : NAMES[row.step];
	const label = (row: StartupRow) => (row.nested ? `  ${name(row)}` : name(row));
	const note = ({ plugin }: StartupRow) =>
		plugin
			? `  ${plugin.core ? 'core' : 'community'}${plugin.error ? `, failed: ${plugin.error}` : ''}`
			: '';
	const width = Math.max(...rows.map((row) => label(row).length)) + 2;

	console.group(
		`%cScratchnote%c ${app.window} window started in %c${duration(total)}%c, ${duration(plugins)} of it waiting on plugins`,
		STYLE.badge,
		'',
		STYLE.total,
		''
	);
	for (const row of rows) {
		const bar = '█'.repeat(Math.round((row.ms / total) * BAR));
		console.info(
			`%c${label(row).padEnd(width)}%c${duration(row.ms).padStart(9)}  %c${bar}%c${note(row)}`,
			row.nested ? STYLE.nested : STYLE.step,
			STYLE.time,
			row.nested ? STYLE.nestedBar : STYLE.bar,
			STYLE.note
		);
	}
	console.groupEnd();
}
