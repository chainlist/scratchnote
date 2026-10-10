import { invoke } from './invoke.js';

/** A file copied into the open space's `attachments/` folder (SPEC 3.7). */
export interface Attachment {
	/** The name it came with, for the link's text. */
	name: string;
	/** From the space's folder: `attachments/2026/2026-09-28 shot.png`. */
	path: string;
}

/** Copies files from disk into the open space, or the one named, in order. */
export const addAttachments = (paths: string[], space?: string) =>
	invoke<Attachment[]>('add_attachments', { paths, space });

/**
 * A pasted file has no path, so its bytes go over as they are, and its name
 * and the space it goes into, when not the open one, in headers.
 */
export const saveAttachment = async (file: File, space?: string) =>
	invoke<Attachment>('save_attachment', new Uint8Array(await file.arrayBuffer()), {
		headers: {
			'x-name': encodeURIComponent(file.name || 'image.png'),
			...(space === undefined ? {} : { 'x-space': encodeURIComponent(space) })
		}
	});

/**
 * Moves attachments from one space into another, for a draft that goes
 * there instead. Resolves to where each one went, in order.
 */
export const moveAttachments = (from: string, to: string, paths: string[]) =>
	invoke<string[]>('move_attachments', { from, to, paths });

/** Opens an attachment of the open space in its own app. */
export const openAttachment = (path: string) => invoke<void>('open_attachment', { path });
