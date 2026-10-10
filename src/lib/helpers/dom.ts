/** Where an event lands, for the gestures and keys the views take for themselves. */

/** Text and fields keep their own touches: a select, and the whole of a note's editor, too. */
export const inText = (target: Element) =>
	target.closest('input, textarea, select, [contenteditable="true"], .cm-editor') !== null;

/** Keys pressed here type: in editable text, a note's editor among it, or a text field. */
export const typesText = (target: HTMLElement) =>
	target.isContentEditable || target.closest('input, textarea') !== null;
