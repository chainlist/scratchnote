import { keymap, EditorView } from '@codemirror/view';
import type { Extension } from '@codemirror/state';
import { openAttachment, openLink } from '#lib/api.js';
import { bold, italic, link } from '#lib/markdown/commands.js';
import { runCommand } from '#lib/plugins/commands.js';
import { registry } from '#lib/plugins/registry.svelte.js';
import { markdownSupport } from './live-preview.js';

/**
 * What the plugins bring to every editor: their syntax, the hotkeys of
 * their commands on the text, and their own CodeMirror extensions.
 */
export function plugged(): Extension {
	const keys = registry.commands
		.filter((command) => command.hotkey && command.editorCallback)
		.map((command) => ({
			key: command.hotkey!,
			run: (view: EditorView) => {
				runCommand(command, view);
				return true;
			}
		}));
	return [
		markdownSupport(),
		keymap.of(keys),
		registry.editorExtensions.map((entry) => entry.extension)
	];
}

/** Ctrl or Cmd and a click opens a link or an attachment; a plain click edits it. */
export const links = EditorView.domEventHandlers({
	mousedown(event) {
		if (!(event.ctrlKey || event.metaKey)) return false;
		const target = (event.target as Element).closest('[data-href], [data-attachment]');
		if (!target) return false;
		event.preventDefault();
		const href = target.getAttribute('data-href');
		if (href) void openLink(href);
		else void openAttachment(target.getAttribute('data-attachment')!);
		return true;
	}
});

/** The toolbar's word-processor keys. */
export const formatKeys = keymap.of([
	{ key: 'Mod-b', run: bold },
	{ key: 'Mod-i', run: italic },
	{ key: 'Mod-k', run: link }
]);

// Type, spacing and colours come from the page, as they did for the textarea.
export const theme = EditorView.theme({
	'&': { flex: '1 1 auto', minHeight: '0' },
	'&.cm-focused': { outline: 'none' },
	'.cm-scroller': { fontFamily: 'inherit', lineHeight: 'inherit' },
	'.cm-content': { padding: '0', minHeight: '100%', caretColor: 'currentColor' },
	'.cm-line': { padding: '0 0 0 var(--md-indent, 0)' },
	'.cm-placeholder': { color: 'var(--color-meta)' },
	// The names offered as `@` is typed, as the app's menus look.
	'.cm-tooltip.cm-tooltip-autocomplete': {
		border: 'none',
		borderRadius: 'var(--radius-lg)',
		padding: '0.25rem',
		background: 'var(--popover)',
		color: 'var(--popover-foreground)',
		// A hairline, not a shadow: the app lifts by tone and border.
		boxShadow: '0 0 0 1px color-mix(in oklab, var(--foreground) 10%, transparent)'
	},
	'.cm-tooltip.cm-tooltip-autocomplete > ul': { fontFamily: 'inherit', maxHeight: '14rem' },
	'.cm-tooltip.cm-tooltip-autocomplete > ul > li': {
		borderRadius: 'var(--radius-md)',
		padding: '0.2rem 0.5rem',
		lineHeight: '1.4'
	},
	'.cm-tooltip.cm-tooltip-autocomplete > ul > li[aria-selected]': {
		background: 'var(--accent)',
		color: 'var(--accent-foreground)'
	},
	'.cm-completionLabel': { fontWeight: '500' },
	'.cm-completionMatchedText': { textDecoration: 'none', color: 'var(--primary)' },
	'.cm-completionDetail': {
		marginLeft: '0.75rem',
		fontStyle: 'normal',
		fontFamily: 'var(--font-mono)',
		fontSize: '0.75em',
		color: 'var(--color-meta)'
	}
});
