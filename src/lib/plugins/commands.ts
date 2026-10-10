import type { EditorView } from '@codemirror/view';
import { errorText } from '#lib/helpers/errors.js';
import { reportError } from './app';
import { Editor } from './editor';
import type { CommandEntry } from './registry.svelte';

/**
 * Run a plugin's command: on the editor it was called from when it acts on
 * text. A command that throws, now or later, says so in the window rather
 * than breaking what called it.
 */
export function runCommand(entry: CommandEntry, cm?: EditorView | null) {
	const fail = (e: unknown) => reportError(`${entry.plugin}: ${errorText(e)}`);
	try {
		const result =
			entry.editorCallback && cm ? entry.editorCallback(new Editor(cm)) : entry.callback?.();
		void Promise.resolve(result).catch(fail);
	} catch (e) {
		fail(e);
	}
}
