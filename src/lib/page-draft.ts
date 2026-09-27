/**
 * A new page not saved yet, since it has no title. It outlives the page view,
 * as the capture window's draft outlives an Esc, and New page brings it back.
 */
export const pageDraft = { title: '', body: '' };

/** Add text handed over from the capture window to whatever the draft holds. */
export function addToPageDraft(body: string) {
	const text = body.trim();
	if (!text) return;
	pageDraft.body = pageDraft.body.trim() ? `${pageDraft.body.trimEnd()}\n\n${text}` : text;
}
