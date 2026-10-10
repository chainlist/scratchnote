<script lang="ts">
	import { onMount } from 'svelte';
	import { history, historyKeymap, standardKeymap } from '@codemirror/commands';
	import { markdownKeymap } from '@codemirror/lang-markdown';
	import { syntaxTree } from '@codemirror/language';
	import {
		Compartment,
		EditorSelection,
		EditorState,
		Prec,
		type StateCommand
	} from '@codemirror/state';
	import { EditorView, keymap, placeholder as placeholderText } from '@codemirror/view';
	import { open as pickFiles } from '@tauri-apps/plugin-dialog';
	import { addAttachments, saveAttachment, type Attachment } from '#lib/api.js';
	import { pickFileBytes } from '#lib/notes/attachments.svelte.js';
	import { acceptDrops } from '#lib/codemirror/drops.js';
	import { formatKeys, links, plugged, theme } from '#lib/codemirror/extensions.js';
	import { attachmentSpace } from '#lib/codemirror/live-preview.js';
	import EditorToolbar from '#lib/components/editor/EditorToolbar.svelte';
	import { errorText } from '#lib/helpers/errors.js';
	import { attachmentLink } from '#lib/markdown.js';
	import { formats } from '#lib/markdown/commands.js';
	import { mentionCompletion } from '#lib/notes/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { android } from '#lib/helpers/platform.js';
	import { Editor } from '#lib/plugins/editor.js';
	import { registry, type ToolbarEntry } from '#lib/plugins/registry.svelte.js';

	let {
		value = $bindable(''),
		placeholder = '',
		label,
		space,
		onerror,
		onattach,
		class: className = '',
		toolbarClass = ''
	}: {
		/** The markdown being edited. */
		value?: string;
		placeholder?: string;
		/** Accessible name of the text field. */
		label: string;
		/** The space files are attached to and drawn from, when not the open one. */
		space?: string;
		/** A file pasted, dropped or picked could not be attached. */
		onerror: (message: string) => void;
		/** Files were attached into `space`, as it was then, and their links written in. */
		onattach?: (attached: Attachment[], space: string | undefined) => void;
		/** Box, padding and type; a max height makes it scroll past it. */
		class?: string;
		/** The toolbar's, such as a background to keep it in view on a long page. */
		toolbarClass?: string;
	} = $props();

	let host: HTMLDivElement;
	let view: EditorView | undefined;
	/** The formats at the cursor, which the toolbar shows pressed. */
	let active = $state({ bold: false, italic: false, bullets: false });
	/** The plugins' buttons shown pressed, of those that can be. */
	let pluginActive = $state.raw<ToolbarEntry[]>([]);
	/** Where the plugins' syntax and keys go, so they change without a new editor. */
	const plugins = new Compartment();
	/** Where `space` goes, so its attachments are drawn from the space it names. */
	const attachedIn = new Compartment();
	const spaceFacet = (name: string | undefined) =>
		name === undefined ? [] : attachmentSpace.of(name);

	/** The formats at the cursor, the app's and the plugins'. */
	function refreshActive(current: EditorView) {
		active = formats(current.state);
		const editor = new Editor(current);
		pluginActive = registry.toolbar.filter((button) => {
			try {
				return button.active?.(editor) ?? false;
			} catch (e) {
				console.error(e);
				return false;
			}
		});
	}

	/** A plugin's button, on the text, which keeps the focus as the app's buttons do. */
	function runPlugin(button: ToolbarEntry) {
		if (!view) return;
		try {
			button.run(new Editor(view));
		} catch (e) {
			onerror(`${button.plugin}: ${errorText(e)}`);
		}
		view.focus();
	}

	/** A toolbar button: the text keeps the focus, so the next one acts on it too. */
	function run(command: StateCommand) {
		if (!view) return;
		command(view);
		view.focus();
	}

	/**
	 * Link what `copy` attaches at the cursor once it is copied in (SPEC 3.7),
	 * into the space the editor names as it starts.
	 */
	async function attach(copy: (into: string | undefined) => Promise<Attachment[]>) {
		const into = space;
		try {
			const attached = await copy(into);
			if (!view) return;
			view.focus();
			if (attached.length === 0) return;
			view.dispatch(view.state.replaceSelection(attached.map(attachmentLink).join('\n')));
			onattach?.(attached, into);
		} catch (e) {
			onerror(m.error_attach({ reason: String(e) }));
		}
	}

	/** A screenshot or a file copied in the file manager is attached rather than pasted. */
	const pasteFiles = EditorView.domEventHandlers({
		paste(event) {
			const data = event.clipboardData;
			const files = [...(data?.files ?? [])];
			// Text wins: an office app puts a picture of the text on the clipboard too.
			if (files.length === 0 || data?.getData('text/plain')) return false;
			event.preventDefault();
			void attach((into) => Promise.all(files.map((file) => saveAttachment(file, into))));
			return true;
		}
	});

	function drop(paths: string[], at: { x: number; y: number }) {
		if (!view) return;
		view.dispatch({ selection: { anchor: view.posAtCoords(at, false) } });
		void attach((into) => addAttachments(paths, into));
	}

	/** Pick files to attach at the cursor, for the paperclip. */
	function attachFiles() {
		void attach(async (into) => {
			if (android) {
				const files = await pickFileBytes();
				return Promise.all(files.map((file) => saveAttachment(file, into)));
			}
			const picked = await pickFiles({ multiple: true });
			return picked ? addAttachments(picked, into) : [];
		});
	}

	onMount(() => {
		view = new EditorView({
			parent: host,
			state: EditorState.create({
				doc: value,
				selection: EditorSelection.cursor(value.length),
				extensions: [
					history(),
					// Enter carries a list or quote on to the next line.
					Prec.high(keymap.of(markdownKeymap)),
					formatKeys,
					// The space's names as `@` is typed (SPEC 3.10).
					mentionCompletion(() => space),
					keymap.of([...standardKeymap, ...historyKeymap]),
					plugins.of(plugged()),
					attachedIn.of(spaceFacet(space)),
					links,
					pasteFiles,
					EditorView.lineWrapping,
					placeholderText(placeholder),
					EditorView.contentAttributes.of({ 'aria-label': label }),
					theme,
					EditorView.updateListener.of((update) => {
						if (update.docChanged) value = update.state.doc.toString();
						if (
							update.docChanged ||
							update.selectionSet ||
							syntaxTree(update.startState) !== syntaxTree(update.state)
						)
							refreshActive(update.view);
					})
				]
			})
		});
		refreshActive(view);
		const stopDrops = acceptDrops(host, drop);
		return () => {
			stopDrops();
			view?.destroy();
		};
	});

	// A plugin switched on or off in either window: its syntax, keys and
	// buttons come or go in the text being written, which stays as it is.
	$effect(() => {
		const extension = plugged();
		void registry.toolbar;
		if (!view) return;
		view.dispatch({ effects: plugins.reconfigure(extension) });
		refreshActive(view);
	});

	// Another space to attach to, as the capture window's draft is sent
	// elsewhere: the images are drawn from there.
	$effect(() => {
		const extension = spaceFacet(space);
		view?.dispatch({ effects: attachedIn.reconfigure(extension) });
	});

	// A value set from outside, such as a draft cleared after saving.
	$effect(() => {
		const next = value;
		if (view && next !== view.state.doc.toString()) {
			view.dispatch({
				changes: { from: 0, to: view.state.doc.length, insert: next },
				selection: { anchor: next.length }
			});
		}
	});

	export function focus() {
		view?.focus();
	}
</script>

<!-- The text goes in after the toolbar. -->
<div bind:this={host} class="md-editor flex flex-col {className}">
	<EditorToolbar
		{active}
		{pluginActive}
		onrun={run}
		onplugin={runPlugin}
		onattach={attachFiles}
		class={toolbarClass}
	/>
</div>
