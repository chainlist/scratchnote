/** Links to attached files (SPEC 3.7), as the notes write them and the cards draw them. */

/** The files an attachment link shows as a picture; the backend serves them as images. */
export const isImage = (path: string) => /\.(png|jpe?g|gif|webp|avif|bmp|svg|ico)$/i.test(path);

/**
 * The way from a note's file up to its space's folder. Day files and page
 * files both sit two folders down, so one link reaches an attachment from
 * either (SPEC 4.8).
 */
const TO_SPACE = '../../';

/**
 * The attached file a link points at, from the space's folder
 * (`attachments/2026/…`), or nothing when it points elsewhere.
 */
export function attachmentPath(url: string): string | undefined {
	let path = url.startsWith('<') && url.endsWith('>') ? url.slice(1, -1) : url;
	try {
		path = decodeURI(path);
	} catch {
		// A stray `%` stays as typed.
	}
	return path.startsWith(`${TO_SPACE}attachments/`) ? path.slice(TO_SPACE.length) : undefined;
}

/** The markdown linking an attachment: an image shows in the text, any other file as a card. */
export function attachmentLink(attachment: { name: string; path: string }): string {
	const text = attachment.name.replace(/[\\`*_[\]]/g, '\\$&');
	return `${isImage(attachment.path) ? '!' : ''}[${text}](<${TO_SPACE}${attachment.path}>)`;
}

/** `text` with the links `attachmentLink` wrote to the file at `from` pointed at `to` instead. */
export function relink(text: string, from: string, to: string): string {
	return text.replaceAll(`(<${TO_SPACE}${from}>)`, `(<${TO_SPACE}${to}>)`);
}

/** The last part of a path, for an attachment linked without a name. */
export const fileName = (path: string) => path.slice(path.lastIndexOf('/') + 1);

/** A file's type as its card shows it: a short extension in capitals, or nothing. */
export const fileType = (path: string) =>
	/\.([a-z0-9]{1,4})$/i.exec(fileName(path))?.[1].toUpperCase() ?? '';

/** A file's name on its card, without the extension its type already shows. */
export function cardName(name: string, path: string): string {
	const type = fileType(path);
	const stem = name.slice(0, -type.length - 1);
	return type && stem && name.toUpperCase().endsWith(`.${type}`) ? stem : name;
}
