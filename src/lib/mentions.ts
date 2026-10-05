import {
	autocompletion,
	type Completion,
	type CompletionContext,
	type CompletionResult
} from '@codemirror/autocomplete';
import { syntaxTree } from '@codemirror/language';
import type { Extension } from '@codemirror/state';
import type { MarkdownConfig } from '@lezer/markdown';
import { goto } from '$app/navigation';
import { resolve } from '$app/paths';
import { listMentions } from '#lib/api.js';
import { daysAgo } from '#lib/days.js';
import type { NodeRender } from '#lib/plugins/api.js';
import { m } from '#lib/paraglide/messages.js';

/**
 * Mentions (SPEC 3.10): `@name` names a project, a person or anything else.
 * The backend reads them by the same rules as this parser (`mentions.rs`),
 * so what shows as a chip is what the notes are listed by.
 */

const AT_SIGN = 64;

/** A letter or digit of a name, in any script, or `_` and `-` inside one. */
const NAME = /[\p{L}\p{N}_-]/u;
/** What a name starts with. */
const FIRST = /[\p{L}\p{N}]/u;

/** `@name`, where the `@` does not follow a letter, so an email stays an email. */
export const mentionSyntax: MarkdownConfig = {
	defineNodes: ['Mention'],
	parseInline: [
		{
			name: 'Mention',
			parse(cx, next, pos) {
				if (next !== AT_SIGN) return -1;
				if (pos > cx.offset && NAME.test(cx.slice(pos - 1, pos))) return -1;
				if (!FIRST.test(cx.slice(pos + 1, pos + 2))) return -1;
				let end = pos + 1;
				while (end < cx.end && NAME.test(cx.slice(end, end + 1))) end++;
				// A name ends on a letter or digit: `@bob-` is `@bob` and a dash.
				while (end > pos + 1 && /[_-]/.test(cx.slice(end - 1, end))) end--;
				return cx.addElement(cx.elt('Mention', pos, end));
			},
			before: 'Link'
		}
	]
};

/** The key a name is found by: case does not count, `@Marie` is `@marie`. */
export const mentionKey = (name: string) => name.replace(/^@/, '').toLowerCase();

/** The hues a name's own colour takes on the lists (oklch), far enough
 *  apart round the wheel to tell eight names apart. */
const HUES = [25, 70, 110, 150, 195, 240, 285, 330];

/** A name's hue on the lists of names and threads, the same every time:
 *  its key, hashed onto `HUES`. */
export function mentionHue(key: string) {
	let hash = 0;
	for (const char of key) hash = (Math.imul(hash, 31) + char.codePointAt(0)!) >>> 0;
	return HUES[hash % HUES.length];
}

/** How many of `days` fall in each of the last `weeks` weeks, the oldest
 *  first, the last of them ending today. */
export function weekly(days: string[], weeks: number) {
	const counts = Array<number>(weeks).fill(0);
	for (const day of days) {
		const week = Math.floor(daysAgo(day) / 7);
		if (week >= 0 && week < weeks) counts[weeks - 1 - week]++;
	}
	return counts;
}

/** The view of the notes mentioning a name. */
export const mentionHref = (name: string) =>
	resolve(`mention/${encodeURIComponent(mentionKey(name))}/`);

/** `text` with `@name` added at its end, as a suggested name is taken. */
export const withMention = (text: string, name: string) => `${text.trimEnd()} @${name}`;

/** A chip; on a card it opens the notes that mention the name. */
export const mentionRender: NodeRender = {
	widget(node) {
		const chip = document.createElement(node.where === 'note' ? 'button' : 'span');
		chip.className = 'mention';
		chip.textContent = node.text;
		if (chip instanceof HTMLButtonElement) {
			chip.type = 'button';
			chip.title = m.mentions_chip_title({ name: node.text });
			chip.addEventListener('click', () => void goto(mentionHref(node.text)));
		}
		return chip;
	}
};

/** Where typing `@` brings up no names: code, and links' targets. */
const QUIET = new Set(['InlineCode', 'CodeText', 'FencedCode', 'CodeBlock', 'URL', 'Autolink']);

/**
 * The names already mentioned, offered as `@` is typed, those pinned to the
 * left edge first, then the most mentioned, so one project keeps one
 * spelling. `space` is the space the text
 * goes into, which has to be the open one for its names to be known.
 */
export function mentionCompletion(space: () => string | undefined): Extension {
	async function names(context: CompletionContext): Promise<CompletionResult | null> {
		const typed = context.matchBefore(/@[\p{L}\p{N}_-]*/u);
		if (!typed) return null;
		const before = context.state.sliceDoc(typed.from - 1, typed.from);
		if (before && NAME.test(before)) return null;
		for (
			let node = syntaxTree(context.state).resolveInner(typed.from, 1);
			node.parent;
			node = node.parent
		)
			if (QUIET.has(node.name)) return null;
		const known = await listMentions(space()).catch(() => []);
		if (context.aborted || known.length === 0) return null;
		const options: Completion[] = known.map((mention, rank) => ({
			label: mention.name,
			detail: String(mention.notes),
			// The pinned first, the projects in hand, then the most
			// mentioned, while what is typed still decides.
			boost: (mention.pinned ? 30 : 0) - Math.min(rank, 20)
		}));
		return { from: typed.from + 1, options, validFor: /^[\p{L}\p{N}_-]*$/u };
	}
	return autocompletion({ override: [names], icons: false });
}
