import { resolve } from '$app/paths';
import type { Pin } from '#lib/api.js';
import { hueOf, mentionHref, mentionHue } from '#lib/mentions.js';
import { threadHref, threadHue } from '#lib/threads.js';

/** A pin's hue (oklch), drawn as a name's tint: a thread's own, a name's
 *  own, or a page's from its type and query. */
export function pinHue(pin: Pin) {
	if (pin.kind === 'thread') return threadHue(pin.target);
	if (pin.kind === 'mention') return mentionHue(pin.target);
	return hueOf(pin.target);
}

/** Two letters for a pin on the left edge: a title's first two words'
 *  initials, or its first two letters, an `@` kept in front. */
export function monogram(label: string) {
	const at = label.startsWith('@');
	const words = label
		.replace(/^[@#]/, '')
		.split(/[^\p{L}\p{N}]+/u)
		.filter(Boolean);
	if (words.length === 0) return at ? '@' : '?';
	const letters =
		words.length > 1 ? [words[0], words[1]].map((word) => [...word][0]) : [...words[0]].slice(0, 2);
	const text = letters.join('');
	return at ? `@${[...text][0].toUpperCase()}` : text.charAt(0).toUpperCase() + text.slice(1);
}

/** The view a pin opens. */
export function pinHref(pin: Pin) {
	if (pin.kind === 'thread') return threadHref(pin.target);
	if (pin.kind === 'mention') return mentionHref(pin.target);
	const at = pin.target.indexOf('?');
	const type = at < 0 ? pin.target : pin.target.slice(0, at);
	return `${resolve(`plugin/${type}/`)}${at < 0 ? '' : pin.target.slice(at)}`;
}

/** The plugin page type a view's pin opens. */
export const pinPage = (pin: Pin) => pin.target.split('?')[0];

/** The same thread or page. */
export const samePin = (a: Pin, b: Pin) => a.kind === b.kind && a.target === b.target;
