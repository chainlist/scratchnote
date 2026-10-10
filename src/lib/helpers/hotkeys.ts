import { mac } from '#lib/helpers/platform.js';

/**
 * Hotkeys in CodeMirror's notation, `Mod-Shift-h`, as the app's and the
 * plugins' commands have them, and accelerators, `CommandOrControl+Shift+Space`,
 * as the global-shortcut plugin reads the capture hotkey.
 */

/** The modifiers and the key of a hotkey in CodeMirror's notation, `Mod-Shift-h`. */
function parse(hotkey: string) {
	const parts = hotkey.split(/-(?!$)/);
	const key = parts.pop() ?? '';
	const mods = new Set(parts.map((part) => part.toLowerCase()));
	return {
		key,
		ctrl: mods.has('ctrl') || (!mac && mods.has('mod')),
		meta: mods.has('meta') || mods.has('cmd') || (mac && mods.has('mod')),
		alt: mods.has('alt'),
		shift: mods.has('shift')
	};
}

export function matchesHotkey(event: KeyboardEvent, hotkey: string): boolean {
	const want = parse(hotkey);
	return (
		event.ctrlKey === want.ctrl &&
		event.metaKey === want.meta &&
		event.altKey === want.alt &&
		event.shiftKey === want.shift &&
		event.key.toLowerCase() === want.key.toLowerCase()
	);
}

/** A hotkey as the command center shows it: `Ctrl+Shift+H`, `⌘⇧H` on macOS. */
export function formatHotkey(hotkey: string): string {
	const { key, ctrl, meta, alt, shift } = parse(hotkey);
	const name = key.length === 1 ? key.toUpperCase() : key;
	if (mac) return `${ctrl ? '⌃' : ''}${alt ? '⌥' : ''}${shift ? '⇧' : ''}${meta ? '⌘' : ''}${name}`;
	return [ctrl && 'Ctrl', meta && 'Win', alt && 'Alt', shift && 'Shift', name]
		.filter(Boolean)
		.join('+');
}

/** Shows an accelerator the way the OS spells it, e.g. Ctrl+Shift+Space. */
export const prettyHotkey = (accelerator: string) =>
	accelerator.replace('CommandOrControl', mac ? '⌘' : 'Ctrl').replace('Super', 'Win');
