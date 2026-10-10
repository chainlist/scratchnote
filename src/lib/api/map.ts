import { event, invoke } from './invoke.js';
import type { Note } from './notes.js';

/** A note's place on the map of the space by meaning (SPEC 6.5), the
 *  smallest category it is in there, if any, and the keys of the names it
 *  mentions. */
export interface MapNote extends Pick<Note, 'id' | 'date' | 'kind'> {
	x: number;
	y: number;
	category: number | null;
	mentions: string[];
}

/** Every note placed on the map. Empty until the embed task has placed them. */
export const noteMap = () => invoke<MapNote[]>('note_map');

/** A group of notes close together on the map, inside its parent, shown from
 *  level `low` to `high`. */
export interface MapCategory {
	id: number;
	parent: number | null;
	low: number;
	high: number;
	name: string;
}

/** The categories of the map: at level k, notes within `base` times √2^k of
 *  each other are in one. */
export interface MapCategories {
	base: number;
	categories: MapCategory[];
}

/** The categories of the map. None until the embed task has found them. */
export const mapCategories = () => invoke<MapCategories>('map_categories');

/** Every note of the map linked to its closest notes, each pair once: the
 *  notes, and each link as two places in `ids`, one after the other. */
export interface MapLinks {
	ids: string[];
	links: number[];
}

export const mapLinks = () => invoke<MapLinks>('map_links');

/** The ids of the notes whose text holds every word, as search finds them,
 *  for the map to light up. */
export const mapSearch = (query: string) => invoke<string[]>('map_search', { query });

/** Fired when notes moved on the map, or joined or left it. */
export const onMapChanged = event('map-changed');
