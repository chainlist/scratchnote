import type { PluginManifest } from '$lib/api';
import type { App, Plugin } from './api';

/**
 * A plugin built into the app (SPEC 3.9), from `src/plugins/`. It is written
 * against the same API as a community plugin and loads the same way; it
 * only ships with the app, is on unless switched off, and follows the
 * interface language through the app's messages.
 */
export interface CorePlugin {
	manifest: PluginManifest;
	/** Its name and description, in the interface language. */
	name: () => string;
	description: () => string;
	plugin: new (app: App, manifest: PluginManifest) => Plugin;
	/** Its stylesheet, added while it is on, as a community plugin's `styles.css`. */
	styles?: string;
}
