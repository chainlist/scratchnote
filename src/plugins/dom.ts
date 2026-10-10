/**
 * What the core plugins share of their own, beside the plugin API: none of
 * it is the app's, so each plugin stays one a community plugin could be,
 * bundling this as it would its own helpers.
 */

/** A new element, with its classes and its text when given. */
export function element<K extends keyof HTMLElementTagNameMap>(
	tag: K,
	className?: string,
	text?: string
): HTMLElementTagNameMap[K] {
	const el = document.createElement(tag);
	if (className) el.className = className;
	if (text !== undefined) el.textContent = text;
	return el;
}
