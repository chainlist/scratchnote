import type { CorePlugin } from '#lib/plugins/core.js';
import basics from './basics';
import journal from './journal';
import stats from './stats';

/**
 * The core plugins (SPEC 3.9): features of the app built as plugins, on the
 * same API a community plugin has, and on unless switched off in Settings >
 * Core plugins, or off until switched on for those that say so. Each folder
 * here is one. A core plugin imports the plugin API (`#lib/plugins/api.js`),
 * the app's messages and UI components, and nothing else of the app, so what
 * it does a community plugin could do too.
 */
export const corePlugins: CorePlugin[] = [basics, stats, journal];
