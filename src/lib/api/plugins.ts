import { event, invoke } from './invoke.js';

/** A plugin's `manifest.json`, as Obsidian's (SPEC 4.9). */
export interface PluginManifest {
	id: string;
	name: string;
	version: string;
	/** The oldest app version it runs on. */
	minAppVersion?: string | null;
	description: string;
	author: string;
	authorUrl?: string | null;
}

/** What `plugins.json` holds: community plugins start off (SPEC 3.9). */
export interface PluginState {
	community: boolean;
	/** Community plugins switched on, by id. */
	enabled: string[];
	/** Core plugins switched off, by id; they are on unless listed. */
	coreDisabled: string[];
	/** Core plugins that start off, switched on, by id. */
	coreEnabled: string[];
}

export interface PluginsView extends PluginState {
	/** Every installed community plugin, by name. */
	installed: PluginManifest[];
}

/** One plugin the registry lists (SPEC 4.10). */
export interface RegistryEntry {
	id: string;
	name: string;
	author: string;
	description: string;
	/** `owner/name` on GitHub. */
	repo: string;
}

export interface PluginDetails {
	/** At the head of its repository: the latest version. */
	manifest: PluginManifest;
	readme: string | null;
}

export const pluginsView = () => invoke<PluginsView>('plugins_view');

/** Every window hears of it and loads or unloads plugins to match. */
export const setPlugins = (plugins: PluginState) => invoke<PluginsView>('set_plugins', { plugins });

/** Refused while community plugins are off or this one is not enabled. */
export const pluginCode = (id: string) =>
	invoke<{ main: string; styles: string | null }>('plugin_code', { id });

/** What the plugin saved last, null before it ever did. */
export const pluginData = (id: string, core: boolean) =>
	invoke<unknown>('plugin_data', { id, core });

export const savePluginData = (id: string, core: boolean, data: unknown) =>
	invoke<void>('save_plugin_data', { id, core, data });

export const browsePlugins = () => invoke<RegistryEntry[]>('browse_plugins');

export const pluginDetails = (repo: string) => invoke<PluginDetails>('plugin_details', { repo });

/** The latest release, over an older one if installed; its data is kept. */
export const installPlugin = (repo: string, id: string) =>
	invoke<PluginsView>('install_plugin', { repo, id });

/** Its files and data go, and it is switched off. */
export const uninstallPlugin = (id: string) => invoke<PluginsView>('uninstall_plugin', { id });

/** Fired to every window when plugins are installed, removed, or switched. */
export const onPluginsChanged = event<PluginsView>('plugins-changed');

/** Fired when a plugin saved its data, with the window that saved it. */
export const onPluginDataChanged = event<{ id: string; window: string }>('plugin-data-changed');
