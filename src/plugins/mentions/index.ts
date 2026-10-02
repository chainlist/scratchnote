import { m } from '#lib/paraglide/messages.js';
import type { CorePlugin } from '#lib/plugins/core.js';
import { MentionsPlugin } from './plugin';
import styles from './styles.css?inline';

const mentions: CorePlugin = {
	manifest: {
		id: 'mentions',
		name: 'Mentions',
		version: '1.0.0',
		description:
			'Write @name to mention someone. A click on a mention lists every note that names them.',
		author: 'Scratchnote'
	},
	name: m.mentions_plugin_name,
	description: m.mentions_plugin_description,
	plugin: MentionsPlugin,
	styles,
	offByDefault: true
};

export default mentions;
