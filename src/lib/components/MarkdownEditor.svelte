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
			const deco = shown(range.from) ? markup : range.bullet ? bullet : hide;
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

	let {
		value = $bindable(''),
		placeholder = '',
		label,
		onerror,
		class: className = ''
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
	} = $props();

	let host: HTMLDivElement;
	let view: EditorView | undefined;

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

	/** Pick files to attach at the cursor, for an attach button. */
	export function attachFiles() {
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
					keymap.of([...standardKeymap, ...historyKeymap]),
					language,
					livePreview,
					links,
					pasteFiles,
					EditorView.lineWrapping,
					placeholderText(placeholder),
					EditorView.contentAttributes.of({ 'aria-label': label }),
					theme,
					EditorView.updateListener.of((update) => {
						if (update.docChanged) value = update.state.doc.toString();
					})
				]
			})
		});
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

<div bind:this={host} class="md-editor flex flex-col {className}"></div>
