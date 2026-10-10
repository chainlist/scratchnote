/**
 * What a line names a note by: its subject, or the start of its text
 * without markup, as a list's mark, a heading's #s or the stars around
 * bold, so a name reads as words.
 */
export function noteTitle(note: { subject: string | null; body: string }) {
	if (note.subject) return note.subject;
	const line = note.body.split('\n').find((text) => text.trim()) ?? '';
	return line
		.replace(/^\s*(?:[-*+]\s+(?:\[[ xX]\]\s+)?|\d+[.)]\s+|#+\s+|>\s*)/, '')
		.replace(/\[([^\]]*)\]\([^)]*\)/g, '$1')
		.replace(/\*\*|__|~~|`/g, '')
		.replace(/\*(\S[^*]*)\*/g, '$1')
		.trim();
}

/** How many words a text holds, as a page's count shows them. */
export const wordCount = (text: string) => text.split(/\s+/).filter(Boolean).length;
