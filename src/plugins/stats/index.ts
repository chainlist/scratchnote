import { m } from '#lib/paraglide/messages.js';
import type { CorePlugin } from '#lib/plugins/core.js';
import { StatsPlugin } from './plugin';
import styles from './styles.css?inline';

const stats: CorePlugin = {
	manifest: {
		id: 'stats',
		name: 'Stats',
		version: '1.0.0',
		description:
			'A panel beside the day with how much the space holds, your writing streak, and a heatmap of the days you wrote.',
		author: 'Scratchnote'
	},
	name: m.stats_plugin_name,
	description: m.stats_plugin_description,
	plugin: StatsPlugin,
	styles,
	offByDefault: true
};

export default stats;
