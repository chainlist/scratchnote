import { m } from '#lib/paraglide/messages.js';
import type { CorePlugin } from '#lib/plugins/core.js';
import { LabelsPlugin } from './plugin.svelte';
import styles from './styles.css?inline';

const labels: CorePlugin = {
	manifest: {
		id: 'labels',
		name: 'Labels',
		version: '1.0.0',
		description:
			"Reads what each note is about from its meaning: a part of life, and for work the kind of job. Shows it on the note's card and lists the notes by label. Needs the search model.",
		author: 'Scratchnote'
	},
	name: m.labels_plugin_name,
	description: m.labels_plugin_description,
	plugin: LabelsPlugin,
	styles,
	offByDefault: true
};

export default labels;
