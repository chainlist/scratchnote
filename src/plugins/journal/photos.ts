import type { MarkdownImage } from '#lib/plugins/api.js';
import { element } from '../dom';
import { between, random } from './paper';

/** The pictures of the notes, stuck on the page as photos rather than drawn in the text. */

/** How a photo is taped in: two corners at the top, two across, or a strip on top. */
const MOUNTS = ['tape-corners', 'tape-diagonal', 'tape-top'] as const;
/** Masking tape most of the time, now and then a washi tape. */
const TAPES = ['#e8ddbb', '#e8ddbb', '#e8ddbb', '#ddb4a8', '#b4cbb8', '#b7c4da', '#e2c88f'];

const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/**
 * A note's text with its pictures taken out, as they are stuck on the page
 * instead. A line that held only pictures goes, bullet and all, and so does
 * a blank line it leaves doubled.
 */
export function withoutImages(body: string, images: MarkdownImage[]): string {
	const MARK = '\u0000';
	let text = '';
	let at = 0;
	for (const image of images) {
		text += body.slice(at, image.from) + MARK;
		at = image.to;
	}
	text += body.slice(at);
	const kept: string[] = [];
	let dropped = false;
	for (const line of text.split('\n')) {
		const bare = line.replaceAll(MARK, '');
		if (line.includes(MARK) && /^\s*([-*+]|\d+[.)])?\s*$/.test(bare)) {
			dropped = true;
			continue;
		}
		if (dropped && bare.trim() === '' && (kept.at(-1) ?? '').trim() === '') continue;
		dropped = false;
		kept.push(bare);
	}
	return kept.join('\n').replace(/\s+$/, '');
}

/** A name that is the file's own, which a photo does not need written under it. */
const isFileName = (name: string) => /\.[a-z0-9]{2,5}$/i.test(name);

/**
 * Where a photo goes: beside the note's text, which wraps round it, alone
 * under it, or in a row of several under it.
 */
type Placement = 'left' | 'right' | 'alone' | 'row';

/** How wide a photo is, as a share of the page's text, by where it goes. */
const WIDTHS: Record<Placement, [number, number]> = {
	left: [40, 50],
	right: [40, 50],
	alone: [55, 75],
	row: [40, 46]
};

/** A picture's file name, the last part of the path its URL loads. */
function fileName(url: string): string {
	let path = url;
	try {
		path = decodeURIComponent(url);
	} catch {
		// A stray `%` stays as it is.
	}
	return path.slice(path.search(/[^/\\]*$/));
}

/**
 * A picture stuck on the page: a print with a white border, askew, taped.
 * How is drawn from the picture's file name, so a picture always sits the
 * same way, whichever note or space it is in.
 */
export function photo(image: MarkdownImage, placement: Placement): HTMLElement {
	const rand = random(fileName(image.url));
	const mount = MOUNTS[Math.floor(rand() * MOUNTS.length)];
	const figure = element('figure', `journal-photo journal-${mount} journal-${placement}`);
	figure.style.setProperty('--width', `${between(rand, ...WIDTHS[placement]).toFixed(1)}%`);
	// Never quite straight: two to seven degrees, either way.
	const tilt = between(rand, 2, 7) * (rand() < 0.5 ? -1 : 1);
	figure.style.setProperty('--tilt', `${tilt.toFixed(1)}deg`);
	const shift = placement === 'alone' ? between(rand, -18, 18) : between(rand, -5, 5);
	figure.style.setProperty('--shift', `${shift.toFixed(1)}%`);
	figure.style.setProperty('--drop', `${between(rand, 0, 0.9).toFixed(2)}rem`);
	figure.style.setProperty('--tape', TAPES[Math.floor(rand() * TAPES.length)]);
	figure.style.setProperty('--tape-tilt', `${between(rand, -6, 6).toFixed(1)}deg`);
	const img = element('img');
	img.src = image.url;
	img.alt = image.name;
	img.draggable = false;
	figure.append(img);
	if (!isFileName(image.name)) figure.append(element('figcaption', 'journal-caption', image.name));
	const pieces = mount === 'tape-top' ? 1 : 2;
	for (let i = 0; i < pieces; i++) figure.append(element('span', 'journal-tape'));
	return figure;
}

/** Wait for the photos in `flow`, which size the text around them. One that cannot load goes. */
export async function settle(flow: HTMLElement) {
	const images = [...flow.querySelectorAll('img')];
	await Promise.all(
		images.map((img) =>
			Promise.race([img.decode().catch(() => img.closest('figure')?.remove()), wait(3000)])
		)
	);
}
