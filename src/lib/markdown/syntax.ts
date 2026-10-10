import {
	Autolink,
	Strikethrough,
	parser as commonmark,
	type MarkdownExtension,
	type MarkdownParser
} from '@lezer/markdown';
import { mentionRender, mentionSyntax } from '#lib/notes/mentions.js';
import { registry, type SyntaxEntry } from '#lib/plugins/registry.svelte.js';
import type { NodeRender } from '#lib/plugins/types.js';

/**
 * The markdown notes are shown with: CommonMark, bare links,
 * ~~strikethrough~~ and `@name` mentions (SPEC 3.10), and what the plugins'
 * syntax adds, such as the Tasks core plugin's `- [ ]` boxes (SPEC 3.9).
 * The editor and the read-only view both go through `preview`, so a note
 * looks the same written and read.
 */
const CORE: MarkdownExtension[] = [Strikethrough, Autolink, mentionSyntax];

/** How the app draws the nodes of its own syntax. */
const CORE_RULES: [string, NodeRender][] = [['Mention', mentionRender]];

interface Syntax {
	from: SyntaxEntry[];
	extensions: MarkdownExtension[];
	parser: MarkdownParser;
	/** How the plugins draw their nodes, by name. */
	rules: Map<string, NodeRender>;
	/** Nodes drawn in a list item's bullet's place, as a task's box is. */
	bulletless: string[];
	/** What a list item may carry after its bullet, for the list commands. */
	itemMarks: RegExp[];
}

let built: Syntax | undefined;

/**
 * The syntax as the plugins have it now, rebuilt when one comes or goes.
 * Read in an effect or a derived, it is followed: a card redraws when a
 * plugin's syntax loads.
 */
export function current(): Syntax {
	const from = registry.syntax;
	if (built?.from === from) return built;
	const extensions = [...CORE];
	const rules = new Map<string, NodeRender>(CORE_RULES);
	const bulletless: string[] = [];
	const itemMarks: RegExp[] = [];
	for (const { syntax } of from) {
		if (syntax.extension) extensions.push(syntax.extension);
		for (const [name, rule] of Object.entries(syntax.render ?? {})) {
			rules.set(name, rule);
			if (rule.replacesBullet) bulletless.push(name);
		}
		itemMarks.push(...(syntax.itemMarks ?? []));
	}
	built = {
		from,
		extensions,
		parser: commonmark.configure(extensions),
		rules,
		bulletless,
		itemMarks
	};
	return built;
}

/** The syntax extensions in use, for the editor's language. */
export const markdownExtensions = () => current().extensions;

/** The tree the cards and the editor read `text` with. */
export const parseMarkdown = (text: string) => current().parser.parse(text);

/** What a list item may carry after its bullet, such as a task's box. */
export const itemMarks = () => current().itemMarks;
