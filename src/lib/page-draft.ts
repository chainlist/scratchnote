/**
 * A new page not saved yet, since it has no title. It outlives the page view,
 * as the capture window's draft outlives an Esc, and New page brings it back.
 */
export const pageDraft = { title: '', body: '' };

/** Text added after a draft's, a blank line between them. */
export function joinText(draft: string, text: string) {
	const added = text.trim();
	if (!added) return draft;
	return draft.trim() ? `${draft.trimEnd()}\n\n${added}` : added;
}

/** Add text handed over from the capture window to whatever the draft holds. */
export function addToPageDraft(body: string) {
	pageDraft.body = joinText(pageDraft.body, body);
}
