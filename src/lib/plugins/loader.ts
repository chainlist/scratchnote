import * as codemirrorLanguage from '@codemirror/language';
import * as codemirrorState from '@codemirror/state';
import * as codemirrorView from '@codemirror/view';
import * as lezerCommon from '@lezer/common';
import * as lezerMarkdown from '@lezer/markdown';
import { getVersion } from '@tauri-apps/api/app';
import { getCurrentWebviewWindow } from '@tauri-apps/api/webviewWindow';
import {
	onPluginDataChanged,
	onPluginsChanged,
	pluginCode,
	pluginsView,
	setPlugins,
	type PluginManifest,
	type PluginState,
	type PluginsView
} from '$lib/api';
import { m } from '$lib/paraglide/messages';
import { corePlugins } from '../../plugins';
import {
	Component,
	createPlugin,
	Editor,
	iconSvg,
	ItemView,
	loadPlugin,
	Plugin,
	PluginSettingTab,
	Setting,
	SettingSection,
	Timeline,
	unloadPlugin,
	type App
} from './api';
import { app, appFor, describeWindow } from './app';
import type { CorePlugin } from './core';
import { plugins } from './state.svelte';

/**
 * Loads the plugins into this window and keeps them in step with what is
 * switched on (SPEC 3.9). Both windows run one: the capture window for the
 * editor's syntax and toolbar, the main window for everything. A change in
 * either goes through the backend, which tells both.
 */

/** What `require('scratchnote')` gives a community plugin. */
const scratchnote = Object.freeze({
	Plugin,
	Component,
	ItemView,
	PluginSettingTab,
	Setting,
	SettingSection,
	Timeline,
	Editor,
	iconSvg
});

/**
 * What a community plugin can `require`, as Obsidian provides `obsidian` and
 * CodeMirror: the app's own copies, so a plugin's syntax and editor
 * extensions are made of the same classes as the editor's.
 */
const MODULES: Record<string, unknown> = {
	scratchnote,
	'@codemirror/language': codemirrorLanguage,
	'@codemirror/state': codemirrorState,
	'@codemirror/view': codemirrorView,
	'@lezer/common': lezerCommon,
	'@lezer/markdown': lezerMarkdown
};

/** How long startup waits on a plugin's async `onload` before it goes on without it. */
const ONLOAD_WAIT_MS = 2000;

export { plugins };

export { corePlugins };
export const coreIds = new Set(corePlugins.map((core) => core.manifest.id));

/** Whether a core plugin is on: as it ships, unless the user switched it. */
export function coreOn(core: CorePlugin, view: PluginState = plugins.view): boolean {
	const id = core.manifest.id;
	return core.offByDefault ? view.coreEnabled.includes(id) : !view.coreDisabled.includes(id);
}

/** A plugin's name as the settings show it: a core one's in the interface language. */
export function pluginName(id: string): string {
	const core = corePlugins.find((candidate) => candidate.manifest.id === id);
	if (core) return core.name();
	return plugins.view.installed.find((manifest) => manifest.id === id)?.name ?? id;
}

export function pluginDescription(id: string): string {
	const core = corePlugins.find((candidate) => candidate.manifest.id === id);
	if (core) return core.description();
	return plugins.view.installed.find((manifest) => manifest.id === id)?.description ?? '';
}

interface Loaded {
	plugin: Plugin;
	version: string;
	style?: HTMLStyleElement;
}

const loaded = new Map<string, Loaded>();

/** Whether version `a` is older than `b`, number by number, as the backend compares. */
export function older(a: string, b: string): boolean {
	const numbers = (v: string) =>
		v
			.split(/[-+]/)[0]
			.split('.')
			.map((n) => Number.parseInt(n, 10) || 0);
	const [x, y] = [numbers(a), numbers(b)];
	for (let i = 0; i < Math.max(x.length, y.length); i++) {
		if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) < (y[i] ?? 0);
	}
	return false;
}

function requireModule(name: string): unknown {
	if (name in MODULES) return MODULES[name];
	throw new Error(`plugins cannot require ${name}: bundle it into main.js`);
}

/**
 * Run a community plugin's `main.js`, a CommonJS module as Obsidian's are,
 * and take the Plugin class it exports. Plugins are trusted code: it runs
 * with the app's own access, which is why community plugins start off.
 */
function evaluate(id: string, source: string) {
	const module = { exports: {} as unknown };
	const run = new Function(
		'module',
		'exports',
		'require',
		`${source}\n//# sourceURL=plugins/${id}/main.js`
	);
	run(module, module.exports, requireModule);
	const exported = (module.exports as { default?: unknown })?.default ?? module.exports;
	if (typeof exported !== 'function' || !(exported.prototype instanceof Plugin))
		throw new Error(m.plugins_error_no_class());
	return exported as new (app: App, manifest: PluginManifest) => Plugin;
}

function addStyle(id: string, css: string) {
	const style = document.createElement('style');
	style.dataset.plugin = id;
	style.textContent = css;
	document.head.append(style);
	return style;
}

/** Wait for `onload`, but not for a plugin that never settles. */
async function start(plugin: Plugin) {
	const result = loadPlugin(plugin);
	if (!(result instanceof Promise)) return;
	let waited: ReturnType<typeof setTimeout> | undefined;
	const late = new Promise<void>((resolve) => (waited = setTimeout(resolve, ONLOAD_WAIT_MS)));
	result.catch((e) => console.error(`${plugin.manifest.id}: onload failed`, e));
	try {
		await Promise.race([result, late]);
	} finally {
		clearTimeout(waited);
	}
}

async function load(
	manifest: PluginManifest,
	Class: new (app: App, manifest: PluginManifest) => Plugin,
	core: boolean,
	styles?: string | null
): Promise<Loaded> {
	const style = styles ? addStyle(manifest.id, styles) : undefined;
	const plugin = createPlugin(Class, appFor(manifest.id), manifest, core);
	try {
		await start(plugin);
	} catch (e) {
		unloadPlugin(plugin);
		style?.remove();
		throw e;
	}
	return { plugin, version: manifest.version, style };
}

const loadCore = (core: CorePlugin) => load(core.manifest, core.plugin, true, core.styles);

async function loadCommunity(manifest: PluginManifest) {
	if (manifest.minAppVersion && older(app.version, manifest.minAppVersion))
		throw new Error(m.plugins_error_needs_app({ version: manifest.minAppVersion }));
	const code = await pluginCode(manifest.id);
	return load(manifest, evaluate(manifest.id, code.main), false, code.styles);
}

function unload(id: string) {
	const entry = loaded.get(id);
	if (!entry) return;
	loaded.delete(id);
	unloadPlugin(entry.plugin);
	entry.style?.remove();
}

/** One timed step of the plugins' startup, for the startup times. */
export interface StartupStep {
	/** Reading the plugin list, loading one plugin, or following changes. */
	step: 'list' | 'plugin' | 'listeners';
	ms: number;
	/** The plugin a `plugin` step loaded, and why it failed. */
	plugin?: { id: string; core: boolean; error?: string };
}

/**
 * When the plugins' startup began and ended, in ms since the window began
 * loading, and its steps, for the startup times (`$lib/startup`).
 */
export const pluginStartup = { start: 0, end: 0, steps: [] as StartupStep[] };

/**
 * Load what is switched on and not loaded, unload what is not, reload what
 * was updated. Each plugin's load is timed into `times` when given.
 */
async function apply(times?: StartupStep[]) {
	const view = plugins.view;
	const wanted = new Map<string, { version: string; load: () => Promise<Loaded> }>();
	for (const core of corePlugins) {
		if (coreOn(core, view))
			wanted.set(core.manifest.id, { version: core.manifest.version, load: () => loadCore(core) });
	}
	if (view.community) {
		for (const manifest of view.installed) {
			if (!view.enabled.includes(manifest.id)) continue;
			// A core plugin's id is the core plugin's.
			const load = coreIds.has(manifest.id)
				? () => Promise.reject(new Error(m.plugins_error_core_id()))
				: () => loadCommunity(manifest);
			if (!wanted.has(manifest.id)) wanted.set(manifest.id, { version: manifest.version, load });
		}
	}

	for (const [id, entry] of [...loaded]) {
		if (wanted.get(id)?.version !== entry.version) unload(id);
	}
	for (const id of Object.keys(plugins.status)) {
		if (!wanted.has(id)) delete plugins.status[id];
	}
	for (const [id, want] of wanted) {
		if (loaded.has(id)) continue;
		const began = performance.now();
		let error: string | undefined;
		try {
			loaded.set(id, await want.load());
			plugins.status[id] = { state: 'on' };
		} catch (e) {
			console.error(`plugin ${id} did not load`, e);
			error = e instanceof Error ? e.message : String(e);
			plugins.status[id] = { state: 'failed', error };
		}
		const plugin = { id, core: coreIds.has(id), error };
		times?.push({ step: 'plugin', ms: performance.now() - began, plugin });
	}
}

let queue = Promise.resolve();

/** One reconcile at a time, each on what the one before left. */
function reconcile(times?: StartupStep[]) {
	queue = queue.then(() => apply(times)).catch((e) => console.error(e));
	return queue;
}

/**
 * Load the plugins before the window's first view, which the client hook
 * awaits, so the first card is already drawn with their syntax.
 */
export async function startPlugins() {
	const { steps } = pluginStartup;
	pluginStartup.start = performance.now();
	let label = 'main';
	try {
		label = getCurrentWebviewWindow().label;
		describeWindow(label, await getVersion());
		plugins.view = await pluginsView();
	} catch (e) {
		console.error('plugins: could not read what is installed', e);
	}
	steps.push({ step: 'list', ms: performance.now() - pluginStartup.start });
	await reconcile(steps);
	const listening = performance.now();
	try {
		await onPluginsChanged((view) => {
			plugins.view = view;
			void reconcile();
		});
		await onPluginDataChanged(({ id, window }) => {
			if (window === label) return;
			try {
				loaded.get(id)?.plugin.onExternalSettingsChange();
			} catch (e) {
				console.error(`${id}: onExternalSettingsChange failed`, e);
			}
		});
	} catch (e) {
		console.error('plugins: could not follow changes', e);
	}
	pluginStartup.end = performance.now();
	steps.push({ step: 'listeners', ms: pluginStartup.end - listening });
}

/** Save a change; the backend tells every window, which load and unload to match. */
async function change(next: Partial<PluginsView>) {
	const { community, enabled, coreDisabled, coreEnabled } = { ...plugins.view, ...next };
	plugins.view = await setPlugins({ community, enabled, coreDisabled, coreEnabled });
	await reconcile();
}

/** A core plugin that starts off is listed once on, any other once off. */
export function setCorePlugin(core: CorePlugin, on: boolean) {
	const id = core.manifest.id;
	const list = core.offByDefault ? 'coreEnabled' : 'coreDisabled';
	const listed = core.offByDefault ? on : !on;
	const others = plugins.view[list].filter((other) => other !== id);
	return change({ [list]: listed ? [...others, id] : others });
}

export const setCommunityPlugin = (id: string, on: boolean) =>
	change({
		enabled: on
			? [...plugins.view.enabled, id]
			: plugins.view.enabled.filter((other) => other !== id)
	});

/** Community plugins on or off as a whole: off, none of their code runs. */
export const setCommunity = (on: boolean) => change({ community: on });

/** Take a new view from an install or an uninstall, which already told every window. */
export function adopt(view: PluginsView) {
	plugins.view = view;
	return reconcile();
}
