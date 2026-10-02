<script module lang="ts">
	import { history, historyKeymap, standardKeymap } from '@codemirror/commands';
	import { commonmarkLanguage, markdownKeymap } from '@codemirror/lang-markdown';
	import { Language, syntaxTree } from '@codemirror/language';
	import {
		Compartment,
		EditorSelection,
		EditorState,
		Facet,
		Prec,
		type Extension,
		type Range
	} from '@codemirror/state';
	import {
		Decoration,
		EditorView,
		ViewPlugin,
		WidgetType,
		keymap,
		placeholder as placeholderText,
		type DecorationSet,
		type ViewUpdate
	} from '@codemirror/view';
	import type { MarkdownExtension, MarkdownParser } from '@lezer/markdown';
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { open as pickFiles } from '@tauri-apps/plugin-dialog';
	import {
		addAttachments,
		openAttachment,
		openLink,
		saveAttachment,
		type Attachment
	} from '#lib/api.js';
	import { attachmentUrl } from '#lib/attachments.svelte.js';
	import { bold, bullets, formats, italic, link } from '#lib/format.js';
	import {
		attachmentLink,
		cardName,
		fileName,
		fileType,
		markdownExtensions,
		preview,
		type WidgetRender
	} from '#lib/markdown.js';
	import { runCommand } from '#lib/plugins/commands.js';
	import { registry } from '#lib/plugins/registry.svelte.js';

	class Bullet extends WidgetType {
		eq() {
			return true;
		}
		toDOM() {
			const span = document.createElement('span');
			span.className = 'md-bullet';
			return span;
		}
	}
	const bullet = Decoration.replace({ widget: new Bullet() });

	/** What a plugin draws for one of its nodes, such as a task's box (SPEC 3.9). */
	class PluginWidget extends WidgetType {
		readonly render: WidgetRender;
		readonly text: string;
		readonly clicks: boolean;
		constructor(render: WidgetRender, text: string, clicks: boolean) {
			super();
			this.render = render;
			this.text = text;
			this.clicks = clicks;
		}
		eq(other: PluginWidget) {
			return other.render === this.render && other.text === this.text;
		}
		toDOM(view: EditorView) {
			const text = this.text;
			let dom: HTMLElement | undefined;
			try {
				dom = this.render({
					text,
					where: 'editor',
					editable: true,
					// Where the node is now, which edits elsewhere may have moved.
					update: (next) => {
						if (!dom) return;
						const from = view.posAtDOM(dom);
						view.dispatch({ changes: { from, to: from + text.length, insert: next } });
					}
				});
			} catch (e) {
				// A faulty plugin leaves the text as typed.
				console.error(e);
				dom = element('span', '', text);
			}
			return dom;
		}
		// A widget that handles its own clicks keeps them from the editor; any
		// other takes the cursor there, which brings its markup back to edit.
		ignoreEvent() {
			return this.clicks;
		}
	}
	const hide = Decoration.replace({});
	const markup = Decoration.mark({ class: 'md-markup' });

	function element(tag: string, className: string, text = '') {
		const node = document.createElement(tag);
		node.className = className;
		node.textContent = text;
		return node;
	}

	/** The space an editor's attachments are in, when it is not the open one. */
	const attachmentSpace = Facet.define<string, string | undefined>({
		combine: (spaces) => spaces[0]
	});

	/** An attachment drawn where its markup is: its image, or the card the read-only view draws. */
	class Attached extends WidgetType {
		readonly path: string;
		readonly name: string;
		readonly image: boolean;
		readonly space: string | undefined;
		constructor(path: string, name: string, image: boolean, space: string | undefined) {
			super();
			this.path = path;
			this.name = name || fileName(path);
			this.image = image;
			this.space = space;
		}
		eq(other: Attached) {
			return (
				other.path === this.path &&
				other.name === this.name &&
				other.image === this.image &&
				other.space === this.space
			);
		}
		toDOM(view: EditorView) {
			if (this.image) {
				const img = document.createElement('img');
				img.className = 'md-image';
				img.src = attachmentUrl(this.path, this.space);
				img.alt = this.name;
				img.dataset.attachment = this.path;
				// Its height is known once it loads, and the editor's lines move for it.
				img.onload = () => view.requestMeasure();
				return img;
			}
			const card = element('span', 'md-file');
			card.title = this.name;
			card.dataset.attachment = this.path;
			card.append(
				element('span', 'md-file-type', fileType(this.path)),
				element('span', 'md-file-name', cardName(this.name, this.path))
			);
			return card;
		}
		// A click on it puts the cursor there, which brings its markup back to edit.
		ignoreEvent() {
			return false;
		}
	}

	function decorate(view: EditorView): DecorationSet {
		const { state } = view;
		const { doc } = state;
		const text = doc.toString();
		const { marks, hidden, lines, attachments, widgets } = preview(syntaxTree(state), text);

		// The lines the cursor or selection is on show their markup, to edit it.
		const shown = (pos: number) => {
			const line = doc.lineAt(pos);
			return (
				view.hasFocus &&
				state.selection.ranges.some((range) => range.from <= line.to && range.to >= line.from)
			);
		};

		const ranges: Range<Decoration>[] = [];
		for (const [from, line] of lines) {
			const attributes = line.style ? { style: line.style } : undefined;
			ranges.push(Decoration.line({ class: line.class, attributes }).range(from));
		}
		for (const mark of marks) {
			const attributes = mark.href ? { 'data-href': mark.href } : undefined;
			ranges.push(Decoration.mark({ class: mark.class, attributes }).range(mark.from, mark.to));
		}
		for (const range of hidden) {
			// A plugin may not hide a line break; a link split over two lines keeps its markup.
			if (range.from === range.to || text.slice(range.from, range.to).includes('\n')) continue;
			const deco = shown(range.from) ? markup : range.bullet ? bullet : hide;
			ranges.push(deco.range(range.from, range.to));
		}
		// A plugin's widget stands in for its node off the line being edited;
		// on that line the node shows as markup, to edit.
		for (const { from, to, text: nodeText, render, clicks } of widgets) {
			if (shown(from)) ranges.push(markup.range(from, to));
			else
				ranges.push(
					Decoration.replace({ widget: new PluginWidget(render, nodeText, clicks) }).range(from, to)
				);
		}
		// On the line being edited an attachment stays in view after its markup.
		const space = state.facet(attachmentSpace);
		for (const attached of attachments) {
			const { from, to } = attached;
			if (text.slice(from, to).includes('\n')) continue;
			const widget = new Attached(attached.path, attached.name, attached.image, space);
			if (shown(from)) {
				ranges.push(markup.range(from, to));
				ranges.push(Decoration.widget({ widget, side: 1 }).range(to));
			} else {
				ranges.push(Decoration.replace({ widget }).range(from, to));
			}
		}
		return Decoration.set(ranges, true);
	}

	const livePreview = () =>
		ViewPlugin.fromClass(
			class {
				decorations: DecorationSet;
				constructor(view: EditorView) {
					this.decorations = decorate(view);
				}
				update(update: ViewUpdate) {
					if (
						update.docChanged ||
						update.selectionSet ||
						update.focusChanged ||
						syntaxTree(update.startState) !== syntaxTree(update.state) ||
						update.startState.facet(attachmentSpace) !== update.state.facet(attachmentSpace)
					)
						this.decorations = decorate(update.view);
				}
			},
			{ decorations: (plugin) => plugin.decorations }
		);

	/**
	 * CommonMark's language data, so the list keys below recognise it, with
	 * the syntax the read-only view parses: the core's and the plugins'.
	 * Both are made again when a plugin's syntax comes or goes, and the live
	 * preview with them, so it draws by the new rules at once.
	 */
	let support: { from: MarkdownExtension[]; extension: Extension } | undefined;
	function markdownSupport(): Extension {
		const from = markdownExtensions();
		if (support?.from !== from) {
			const parser = (commonmarkLanguage.parser as MarkdownParser).configure(from);
			const language = new Language(commonmarkLanguage.data, parser, [], 'markdown');
			support = { from, extension: [language, livePreview()] };
		}
		return support.extension;
	}

	/**
	 * What the plugins bring to every editor: their syntax, the hotkeys of
	 * their commands on the text, and their own CodeMirror extensions.
	 */
	function plugged(): Extension {
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
	const links = EditorView.domEventHandlers({
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
	const formatKeys = keymap.of([
		{ key: 'Mod-b', run: bold },
		{ key: 'Mod-i', run: italic },
		{ key: 'Mod-k', run: link }
	]);

	/** What each editor's box, `.md-editor`, does with files dropped on it. */
	const dropTargets = new WeakMap<
		Element,
		(paths: string[], at: { x: number; y: number }) => void
	>();
	let listening = false;

	/**
	 * Files dragged in from the file manager go to Tauri rather than the page,
	 * with their paths, so one listener per window finds the editor under the
	 * pointer, outlines it while they hover, and hands it the drop.
	 */
	function listenForDrops() {
		if (listening) return;
		listening = true;
		void getCurrentWebview().onDragDropEvent(({ payload }) => {
			for (const box of document.querySelectorAll('.md-drop')) box.classList.remove('md-drop');
			if (payload.type === 'leave') return;
			const at = payload.position.toLogical(window.devicePixelRatio);
			const box = document.elementFromPoint(at.x, at.y)?.closest('.md-editor');
			const drop = box && dropTargets.get(box);
			if (!drop) return;
			if (payload.type === 'drop') drop(payload.paths, at);
			else box.classList.add('md-drop');
		});
	}

	// Type, spacing and colours come from the page, as they did for the textarea.
	const theme = EditorView.theme({
		'&': { flex: '1 1 auto', minHeight: '0' },
		'&.cm-focused': { outline: 'none' },
		'.cm-scroller': { fontFamily: 'inherit', lineHeight: 'inherit' },
		'.cm-content': { padding: '0', minHeight: '100%', caretColor: 'currentColor' },
		'.cm-line': { padding: '0 0 0 var(--md-indent, 0)' },
		'.cm-placeholder': { color: 'var(--color-neutral-600)' }
	});
</script>

<script lang="ts">
	import { onMount } from 'svelte';
	import type { StateCommand } from '@codemirror/state';
	import BoldIcon from '@lucide/svelte/icons/bold';
	import ItalicIcon from '@lucide/svelte/icons/italic';
	import LinkIcon from '@lucide/svelte/icons/link';
	import ListIcon from '@lucide/svelte/icons/list';
	import PaperclipIcon from '@lucide/svelte/icons/paperclip';
	import PluginIcon from '#lib/components/PluginIcon.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Separator } from '#lib/components/ui/separator/index.js';
	import { Toggle } from '#lib/components/ui/toggle/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { Editor } from '#lib/plugins/editor.js';
	import { labelText, type ToolbarEntry } from '#lib/plugins/registry.svelte.js';

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
			onerror(`${button.plugin}: ${e instanceof Error ? e.message : String(e)}`);
		}
		view.focus();
	}

	// See-through greys, so the buttons show on every editor's background.
	const tool =
		'size-6 min-w-6 px-0 text-neutral-500 hover:bg-neutral-500/15 hover:text-neutral-200 dark:hover:bg-neutral-500/15 aria-pressed:bg-neutral-500/25 aria-pressed:text-neutral-100 data-[state=on]:bg-neutral-500/25';
	const divider = 'mx-1 h-4 bg-neutral-500/30 data-vertical:self-center';

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
			onerror(String(e));
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
		dropTargets.set(host, drop);
		listenForDrops();
		return () => {
			dropTargets.delete(host);
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
	<!-- For those who do not write markdown (SPEC 3.4). A press keeps the
	     focus, and so the selection, in the text; its buttons are what take
	     the focus from the keyboard. -->
	<!-- svelte-ignore a11y_interactive_supports_focus -->
	<div
		role="toolbar"
		aria-label={m.format_toolbar()}
		onmousedown={(event) => event.preventDefault()}
		class="mb-1 -ml-1 flex shrink-0 items-center gap-0.5 {toolbarClass}"
	>
		<Toggle
			size="sm"
			class={tool}
			bind:pressed={() => active.bold, () => run(bold)}
			aria-label={m.format_bold()}
			title={m.format_bold()}
		>
			<BoldIcon class="size-3.5" />
		</Toggle>
		<Toggle
			size="sm"
			class={tool}
			bind:pressed={() => active.italic, () => run(italic)}
			aria-label={m.format_italic()}
			title={m.format_italic()}
		>
			<ItalicIcon class="size-3.5" />
		</Toggle>
		{@render pluginTools('text')}
		<Separator orientation="vertical" class={divider} />
		<Toggle
			size="sm"
			class={tool}
			bind:pressed={() => active.bullets, () => run(bullets)}
			aria-label={m.format_bullets()}
			title={m.format_bullets()}
		>
			<ListIcon class="size-3.5" />
		</Toggle>
		{@render pluginTools('lists')}
		<Separator orientation="vertical" class={divider} />
		<Button
			variant="ghost"
			size="icon-xs"
			class={tool}
			onclick={() => run(link)}
			aria-label={m.format_link()}
			title={m.format_link()}
		>
			<LinkIcon class="size-3.5" />
		</Button>
		<Button
			variant="ghost"
			size="icon-xs"
			class={tool}
			onclick={attachFiles}
			aria-label={m.attach_file()}
			title={m.attach_file()}
		>
			<PaperclipIcon class="size-3.5" />
		</Button>
		{@render pluginTools('insert')}
	</div>
</div>

<!-- The plugins' buttons in a group, such as the Tasks plugin's checklist
     beside the bulleted list (SPEC 3.9). -->
{#snippet pluginTools(group: 'text' | 'lists' | 'insert')}
	{#each registry.toolbar.filter((button) => (button.group ?? 'insert') === group) as button (button)}
		{@const title = labelText(button.title)}
		{#if button.active}
			<Toggle
				size="sm"
				class={tool}
				bind:pressed={() => pluginActive.includes(button), () => runPlugin(button)}
				aria-label={title}
				{title}
			>
				<PluginIcon icon={button.icon} class="size-3.5" />
			</Toggle>
		{:else}
			<Button
				variant="ghost"
				size="icon-xs"
				class={tool}
				onclick={() => runPlugin(button)}
				aria-label={title}
				{title}
			>
				<PluginIcon icon={button.icon} class="size-3.5" />
			</Button>
		{/if}
	{/each}
{/snippet}
