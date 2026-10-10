import { event, invoke } from './invoke.js';

/** A thread, a name mentioned or a plugin's page pinned to the left edge of
 *  a space (SPEC 3.13). */
export interface Pin {
	kind: 'thread' | 'mention' | 'view';
	/** A thread's id, a name's key, or a page's type and query: `journal?date=2026-09-28`. */
	target: string;
	/** A name as typed, or a page's title when it was pinned. A thread's is read live. */
	label?: string | null;
}

/** What the open space has pinned, in its order. */
export const listPins = () => invoke<Pin[]>('list_pins');

/** Pin these in place of the pins before. Resolves to them as saved. */
export const setPins = (pins: Pin[]) => invoke<Pin[]>('set_pins', { pins });

/** Fired when the pins of a space changed, from either window. */
export const onPinsChanged = event('pins-changed');
