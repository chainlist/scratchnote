import { event, invoke } from './invoke.js';

export interface SpaceSummary {
	/** Also the space's folder name under `spaces/`. */
	name: string;
	/**
	 * How many notes it holds, counted when it was last open. Null for a space
	 * never open since, such as a folder made by hand.
	 */
	notes: number | null;
}

export interface SpacesView {
	/** The open space, which every note command acts on. */
	active: string;
	/** By name. */
	spaces: SpaceSummary[];
}

export const listSpaces = () => invoke<SpacesView>('list_spaces');

/** Makes the space and opens it. */
export const createSpace = (name: string) => invoke<SpacesView>('create_space', { name });

/** Renames the space's folder too. */
export const renameSpace = (name: string, newName: string) =>
	invoke<SpacesView>('rename_space', { name, newName });

/** Moves the space's folder to `.scratchnote/trash/`, so nothing is lost. */
export const deleteSpace = (name: string) => invoke<SpacesView>('delete_space', { name });

export const setActiveSpace = (name: string) => invoke<SpacesView>('set_active_space', { name });

/** Shows the space's folder in the system file manager. */
export const openSpaceFolder = (name: string) => invoke<void>('open_space_folder', { name });

/** Fired to every window when a space is opened, made, renamed or deleted. */
export const onSpacesChanged = event<SpacesView>('spaces-changed');
