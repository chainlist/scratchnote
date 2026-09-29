import type { CorePlugin } from '$lib/plugins/core';
import basics from './basics';

/**
 * The core plugins (SPEC 3.9): features of the app built as plugins, on the
 * same API a community plugin has, and on unless switched off in Settings >
 * Plugins. Each folder here is one. A core plugin imports the plugin API
 * (`$lib/plugins/api`), the app's messages and UI components, and nothing
 * else of the app, so what it does a community plugin could do too.
 */
export const corePlugins: CorePlugin[] = [basics];
