import { OverlayScrollbars } from 'overlayscrollbars';
import type { Attachment } from 'svelte/attachments';

/**
 * Draws the element's scrollbars over its content, so they take no room and
 * the layout keeps its width whether it scrolls or not. The element stays the
 * one that scrolls, with no wrapper around its children: the scrollbars are
 * added at its end. Where the system already draws them over the page, as a
 * phone does, the native ones stay.
 */
export const scrollbars: Attachment<HTMLElement> = (node) => {
	const instance = OverlayScrollbars(
		{ target: node, elements: { viewport: node }, cancel: { nativeScrollbarsOverlaid: true } },
		{ scrollbars: { theme: 'os-theme-app', autoHide: 'move' } }
	);
	return () => instance.destroy();
};
