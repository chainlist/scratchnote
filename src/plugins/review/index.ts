import { m } from '$lib/paraglide/messages';
import type { CorePlugin } from '$lib/plugins/core';
import { ReviewPlugin } from './plugin';
import styles from './styles.css?inline';

const review: CorePlugin = {
	manifest: {
		id: 'review',
		name: 'Weekly review',
		version: '1.0.0',
		description:
			"A week on one page: its notes by category, the tasks ticked and still open, and the week's numbers.",
		author: 'Scratchnote'
	},
	name: m.review_plugin_name,
	description: m.review_plugin_description,
	plugin: ReviewPlugin,
	styles,
	offByDefault: true
};

export default review;
