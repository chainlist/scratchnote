import { convertFileSrc } from '@tauri-apps/api/core';
import { listSpaces, onSpacesChanged } from '$lib/api';

/** The open space, whose attachments the notes on show link to. */
let space = $state('');

/** Keep up with the open space in this window. Resolves to the way to stop. */
export function followSpace() {
	void listSpaces().then((view) => (space = view.active));
	return onSpacesChanged((view) => (space = view.active));
}

/**
 * Where the webview loads an attachment of the open space from, or of
 * `inSpace`: the backend's `attachment` protocol. The space is in the path,
 * so files of the same name in two spaces are never taken for one another's
 * cached image.
 */
export const attachmentUrl = (path: string, inSpace = space) =>
	convertFileSrc(`${inSpace}/${path}`, 'attachment');
