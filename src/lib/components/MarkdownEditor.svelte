<script module lang="ts">
	import { history, historyKeymap, standardKeymap } from '@codemirror/commands';
	import { commonmarkLanguage, markdownKeymap } from '@codemirror/lang-markdown';
	import { Language, syntaxTree } from '@codemirror/language';
	import { EditorSelection, EditorState, Prec, type Range } from '@codemirror/state';
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
	import type { MarkdownParser } from '@lezer/markdown';
	import { getCurrentWebview } from '@tauri-apps/api/webview';
	import { open as pickFiles } from '@tauri-apps/plugin-dialog';
	import {
		addAttachments,
		openAttachment,
		openLink,
		saveAttachment,
		type Attachment
	} from '$lib/api';
	import { attachmentUrl } from '$lib/attachments.svelte';
	import { bold, bullets, checklist, formats, italic, link } from '$lib/format';
	import { attachmentLink, cardName, fileName, fileType, preview, syntax } from '$lib/markdown';

	// CommonMark's language data, so the list keys below recognise it, with
	// the same syntax the read-only view parses.
	const language = new Language(
		commonmarkLanguage.data,
		(commonmarkLanguage.parser as MarkdownParser).configure(syntax),
		[],
		'markdown'
	);

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

	/** A task's box, drawn as the read-only view draws it. */
	class TaskBox extends WidgetType {
		readonly done: boolean;
		constructor(done: boolean) {
			super();
			this.done = done;
		}
		eq(other: TaskBox) {
			return other.done === this.done;
		}
		toDOM() {
			const box = document.createElement('span');
			box.className = this.done ? 'md-task md-ticked' : 'md-task';
			return box;
		}
		// `taskBoxes`, below, handles its click.
		ignoreEvent() {
			return false;
		}
	}
	const openBox = Decoration.replace({ widget: new TaskBox(false) });
	const tickedBox = Decoration.replace({ widget: new TaskBox(true) });
	const hide = Decoration.replace({});
	const markup = Decoration.mark({ class: 'md-markup' });

	function element(tag: string, className: string, text = '') {
		const node = document.createElement(tag);
		node.className = className;
		node.textContent = text;
		return node;
	}

	/** An attachment drawn where its markup is: its image, or the card the read-only view draws. */
	class Attached extends WidgetType {
		readonly path: string;
		readonly name: string;
		readonly image: boolean;
		constructor(path: string, name: string, image: boolean) {
			super();
			this.path = path;
			this.name = name || fileName(path);
			this.image = image;
		}
		eq(other: Attached) {
			return other.path === this.path && other.name === this.name && other.image === this.image;
		}
		toDOM(view: EditorView) {
			if (this.image) {
				const img = document.createElement('img');
				img.className = 'md-image';
				img.src = attachmentUrl(this.path);
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
		const { marks, hidden, lines, attachments } = preview(syntaxTree(state), text);

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
			const deco = shown(range.from)
				? markup
				: range.task
					? range.task.done
						? tickedBox
						: openBox
					: range.bullet
						? bullet
						: hide;
			ranges.push(deco.range(range.from, range.to));
		}
		// On the line being edited an attachment stays in view after its markup.
		for (const attached of attachments) {
			const { from, to } = attached;
			if (text.slice(from, to).includes('\n')) continue;
			const widget = new Attached(attached.path, attached.name, attached.image);
			if (shown(from)) {
				ranges.push(markup.range(from, to));
				ranges.push(Decoration.widget({ widget, side: 1 }).range(to));
			} else {
				ranges.push(Decoration.replace({ widget }).range(from, to));
			}
		}
		return Decoration.set(ranges, true);
	}

	const livePreview = ViewPlugin.fromClass(
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
					syntaxTree(update.startState) !== syntaxTree(update.state)
				)
					this.decorations = decorate(update.view);
			}
		},
		{ decorations: (plugin) => plugin.decorations }
	);

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

	/** A click on a task's box ticks or clears it, and leaves the cursor where it was. */
	const taskBoxes = EditorView.domEventHandlers({
		mousedown(event, view) {
			const box = (event.target as Element).closest('.md-task');
			if (!box) return false;
			event.preventDefault();
			const at = view.posAtDOM(box) + 1;
			const done = view.state.sliceDoc(at, at + 1) !== ' ';
			view.dispatch({ changes: { from: at, to: at + 1, insert: done ? ' ' : 'x' } });
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
	import ListTodoIcon from '@lucide/svelte/icons/list-todo';
	import PaperclipIcon from '@lucide/svelte/icons/paperclip';
	import { Button } from '$lib/components/ui/button';
	import { Separator } from '$lib/components/ui/separator';
	import { Toggle } from '$lib/components/ui/toggle';
	import { m } from '$lib/paraglide/messages';

	let {
		value = $bindable(''),
		placeholder = '',
		label,
		onerror,
		class: className = '',
		toolbarClass = ''
	}: {
		/** The markdown being edited. */
		value?: string;
		placeholder?: string;
		/** Accessible name of the text field. */
		label: string;
		/** A file pasted, dropped or picked could not be attached. */
		onerror: (message: string) => void;
		/** Box, padding and type; a max height makes it scroll past it. */
		class?: string;
		/** The toolbar's, such as a background to keep it in view on a long page. */
		toolbarClass?: string;
	} = $props();

	let host: HTMLDivElement;
	let view: EditorView | undefined;
	/** The formats at the cursor, which the toolbar shows pressed. */
	let active = $state({ bold: false, italic: false, bullets: false, checklist: false });

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

	/** Link what `pending` attaches at the cursor once it is copied in (SPEC 3.7). */
	async function attach(pending: Promise<Attachment[]>) {
		try {
			const attached = await pending;
			if (!view) return;
			view.focus();
			if (attached.length === 0) return;
			view.dispatch(view.state.replaceSelection(attached.map(attachmentLink).join('\n')));
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
			void attach(Promise.all(files.map(saveAttachment)));
			return true;
		}
	});

	function drop(paths: string[], at: { x: number; y: number }) {
		if (!view) return;
		view.dispatch({ selection: { anchor: view.posAtCoords(at, false) } });
		void attach(addAttachments(paths));
	}

	/** Pick files to attach at the cursor, for the paperclip. */
	function attachFiles() {
		void attach(
			(async () => {
				const picked = await pickFiles({ multiple: true });
				return picked ? addAttachments(picked) : [];
			})()
		);
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
					language,
					livePreview,
					links,
					taskBoxes,
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
							active = formats(update.state);
					})
				]
			})
		});
		active = formats(view.state);
		dropTargets.set(host, drop);
		listenForDrops();
		return () => {
			dropTargets.delete(host);
			view?.destroy();
		};
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
		<Toggle
			size="sm"
			class={tool}
			bind:pressed={() => active.checklist, () => run(checklist)}
			aria-label={m.format_checklist()}
			title={m.format_checklist()}
		>
			<ListTodoIcon class="size-3.5" />
		</Toggle>
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
	</div>
</div>
