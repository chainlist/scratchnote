import { invoke } from './invoke.js';

/** A label, and how sure of it the classifier is, from 0 to 1. */
export interface Guess {
	label: string;
	score: number;
}

/** What a note is about (SPEC 3.14): its part of life, and for work, its job family. */
export interface NoteLabel {
	life: Guess;
	job?: Guess;
}

/** Every embedded note's label, by id. Empty without the embedding model. */
export const noteLabels = () => invoke<Record<string, NoteLabel>>('note_labels');
