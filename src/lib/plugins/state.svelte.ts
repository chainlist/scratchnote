import type { PluginsView } from '$lib/api';

export type PluginStatus = { state: 'on' } | { state: 'failed'; error: string };

/** What the settings show of the plugins: the loader keeps it up to date. */
class Plugins {
	/** What is installed and switched on, as the backend has it. */
	view = $state<PluginsView>({ community: false, enabled: [], coreDisabled: [], installed: [] });
	/** How each plugin switched on fared in this window, by id. */
	status = $state<Record<string, PluginStatus>>({});
}

export const plugins = new Plugins();
