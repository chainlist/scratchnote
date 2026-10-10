import { matchesHotkey } from '#lib/hotkeys.js';
import { mac } from '#lib/platform.js';
import { runCommand } from '#lib/plugins/commands.js';
import { registry } from '#lib/plugins/registry.svelte.js';
import { typesText } from '#lib/dom.js';
import type { Shell } from '#lib/shell.svelte.js';

/**
 * A plugin's command with this hotkey, run. Those on the text are the
 * editor's own keys, and one the editor already took is left to it.
 */
function runPluginHotkey(event: KeyboardEvent) {
	if (event.defaultPrevented) return false;
	// A key without a modifier types, in text.
	const typing = typesText(event.target as HTMLElement);
	if (typing && !(event.ctrlKey || event.metaKey || event.altKey)) return false;
	const command = registry.commands.find(
		(entry) => entry.hotkey && entry.callback && matchesHotkey(event, entry.hotkey)
	);
	if (command) runCommand(command);
	return command !== undefined;
}

/** The main window's keys: the command center, the settings and the plugins' hotkeys. */
export const windowKeydown = (shell: Shell) => (event: KeyboardEvent) => {
	if (event.key === '/' && (event.ctrlKey || event.metaKey)) {
		event.preventDefault();
		if (shell.paletteOpen) shell.paletteOpen = false;
		else shell.openPalette();
	} else if (matchesHotkey(event, 'Mod-,')) {
		event.preventDefault();
		shell.paletteOpen = false;
		shell.settingsOpen = true;
	} else if (runPluginHotkey(event)) {
		event.preventDefault();
	} else if (!mac && event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
		// Alt+arrows take the webview through its history, which holds the
		// views. In text they would leave a note or a page half written.
		// On macOS they move by word instead, and the text keeps them.
		if (typesText(event.target as HTMLElement)) event.preventDefault();
	}
};
