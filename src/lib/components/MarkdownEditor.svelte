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
	import { openLink } from '$lib/api';
	import { preview, syntax } from '$lib/markdown';

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

	function decorate(view: EditorView): DecorationSet {
		const { state } = view;
		const { doc } = state;
		const text = doc.toString();
		const { marks, hidden, lines } = preview(syntaxTree(state), text);

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

	/** Ctrl or Cmd and a click opens a link; a plain click edits it. */
	const links = EditorView.domEventHandlers({
		mousedown(event) {
			if (!(event.ctrlKey || event.metaKey)) return false;
			const href = (event.target as Element).closest('[data-href]')?.getAttribute('data-href');
			if (!href) return false;
			event.preventDefault();
			void openLink(href);
			return true;
		}
	});

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
		class: className = ''
	}: {
		/** The markdown being edited. */
		value?: string;
		placeholder?: string;
		/** Accessible name of the text field. */
		label: string;
		/** Box, padding and type; a max height makes it scroll past it. */
		class?: string;
	} = $props();

	let host: HTMLDivElement;
	let view: EditorView | undefined;

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
		return () => view?.destroy();
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

<div bind:this={host} class="flex flex-col {className}"></div>
