import { m } from '#lib/paraglide/messages.js';
import type { CorePlugin } from '#lib/plugins/core.js';
import highlightStyles from './highlights/styles.css?inline';
import { BasicsPlugin } from './plugin.svelte';
import taskStyles from './tasks/styles.css?inline';

const basics: CorePlugin = {
	manifest: {
		id: 'basics',
		name: 'Basics',
		version: '1.0.0',
		description: 'Tasks to tick and text to highlight, each with its own switch in the settings.',
		author: 'Scratchnote'
	},
	name: m.basics_plugin_name,
	description: m.basics_plugin_description,
	plugin: BasicsPlugin,
	styles: taskStyles + highlightStyles
};

export default basics;
