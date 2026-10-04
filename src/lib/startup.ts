import { dev } from '$app/env';
import { launchSteps, startupCalls, type StartupCall } from '#lib/api.js';
import { app } from '#lib/plugins/app.js';
import { pluginName, pluginStartup, type StartupStep } from '#lib/plugins/loader.js';

/**
 * How long this window took to start: loading the app's code, the wait on
 * the plugins with each plugin's load, and the first view. Times run from
 * when the window began loading. The root layout records them once the
 * first view is up; the console shows them, and so does Settings > General.
 *
 * A dev build says more: what the app did at launch before the main window
 * began loading, how loading the app split between fetching the page,
 * fetching the code and running it, and which commands the first view waited
 * on. Those rows carry their own English label.
 */

/** A phase of the startup, or a step of the wait on the plugins. */
export interface StartupRow {
	step: 'app' | 'plugins' | 'view' | 'detail' | StartupStep['step'];
	ms: number;
	/** A step of the phase above, shown under it. */
	nested: boolean;
	plugin?: StartupStep['plugin'];
	/** A dev build's detail row says what it is itself. */
	label?: string;
}

export interface StartupTimes {
	/**
	 * From when the window began loading until its first view was up; in a
	 * dev build's main window, from when the app was launched.
	 */
	total: number;
	/** How much of it the first view waited on the plugins. */
	plugins: number;
	rows: StartupRow[];
}

let times: StartupTimes | null = null;

/** This window's startup times, once its first view is up; null before. */
export const startupTimes = () => times;

/** Keep the startup times, now that the first view is up, and log them. */
export async function recordStartup() {
	const total = performance.now();
	startupCalls.open = false;
	const { start, end, steps } = pluginStartup;
	const rows: StartupRow[] = [
		{ step: 'app', ms: start, nested: false },
		...(dev ? loading(start) : []),
		{ step: 'plugins', ms: end - start, nested: false },
		...steps.map((step) => ({ ...step, nested: true })),
		{ step: 'view', ms: total - end, nested: false },
		...(dev ? firstView(end, total) : [])
	];
	const before = dev && app.window === 'main' ? await launch() : [];
	const launched = before[0]?.ms ?? 0;
	times = { total: launched + total, plugins: end - start, rows: [...before, ...rows] };
	log(times);
}

const detail = (label: string, ms: number, nested = true): StartupRow => ({
	step: 'detail',
	label,
	ms: Math.max(0, ms),
	nested
});

/**
 * What the app did at launch before this window began loading, as the
 * backend timed it. Nothing for a window reloaded later, which did not wait
 * on it.
 */
async function launch(): Promise<StartupRow[]> {
	const launch = await launchSteps().catch(() => null);
	if (!launch) return [];
	const waited = performance.timeOrigin - launch.began;
	const rows = launch.steps.map(({ step, ms }) => detail(step, ms));
	// The webview is made and starts loading once the setup is over.
	const rest = waited - launch.steps.reduce((sum, { ms }) => sum + ms, 0);
	if (rest > 0) rows.push(detail('Opening the window', rest));
	return [detail('Before the window', waited, false), ...rows];
}

/**
 * Loading the app, from the browser's own timings: the page, then the code
 * it imports, then running that code until the plugins start.
 */
function loading(start: number): StartupRow[] {
	const [page] = performance.getEntriesByType('navigation') as PerformanceNavigationTiming[];
	if (!page) return [];
	const files = (performance.getEntriesByType('resource') as PerformanceResourceTiming[]).filter(
		(file) => file.startTime < start
	);
	const fetched = Math.max(page.responseEnd, ...files.map((file) => file.responseEnd));
	return [
		detail('Fetching the page', page.responseEnd),
		detail(`Fetching the code (${files.length} files)`, fetched - page.responseEnd),
		detail('Running the code', start - fetched)
	];
}

/**
 * The first view: the wait for its first command, which SvelteKit spends
 * starting its router and the view's code, each command with when it began
 * (they may overlap), then drawing the view once the last answer came.
 */
function firstView(end: number, total: number): StartupRow[] {
	const calls = startupCalls.calls
		.filter((call) => call.start >= end)
		.sort((a, b) => a.start - b.start);
	if (calls.length === 0) return [];
	const answered = Math.max(...calls.map((call) => call.start + call.ms));
	const at = (call: StartupCall) => `at +${Math.round(call.start - end)} ms`;
	return [
		detail('Until the first command', calls[0].start - end),
		...calls.map((call) => detail(`Command ${call.cmd} (${at(call)})`, call.ms)),
		detail('Drawing the view', total - answered)
	];
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
const NAMES: Record<Exclude<StartupRow['step'], 'plugin' | 'detail'>, string> = {
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
		row.label ??
		(row.step === 'plugin' ? pluginName(row.plugin!.id) : NAMES[row.step as keyof typeof NAMES]);
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
