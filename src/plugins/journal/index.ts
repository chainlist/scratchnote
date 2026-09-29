import { m } from '$lib/paraglide/messages';
import type { CorePlugin } from '$lib/plugins/core';
import { JournalPlugin } from './plugin';
import styles from './styles.css?inline';

const journal: CorePlugin = {
	manifest: {
		id: 'journal',
		name: 'Journal view',
		version: '1.0.0',
		description:
			'Your days as a journal book. Turn the pages from one day to the next, with the pictures of your notes taped in.',
		author: 'Scratchnote'
	},
	name: m.journal_plugin_name,
	description: m.journal_plugin_description,
	plugin: JournalPlugin,
	styles,
	offByDefault: true
};

export default journal;
