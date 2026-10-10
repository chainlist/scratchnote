import { readJson, writeJson } from '#lib/storage.js';

/**
 * Which side of the view the dock sits on and how wide it is, kept between
 * launches. It is the window's layout rather than a setting. Storage can be
 * missing, in which case the dock opens on the right at its default width.
 */
const KEY = 'dock';

export type DockSide = 'left' | 'right';

interface DockLayout {
	side: DockSide;
	/** A percentage of the room beside the ribbon. */
	size: number;
}

/** The narrowest and widest the dock goes, as percentages. */
export const DOCK_MIN = 20;
export const DOCK_MAX = 70;

const DEFAULT: DockLayout = { side: 'right', size: 40 };

export function loadDock(): DockLayout {
	const saved = readJson(KEY) as { side?: unknown; size?: unknown } | null;
	const size = Number(saved?.size);
	return {
		side: saved?.side === 'left' ? 'left' : 'right',
		size: Number.isFinite(size) ? Math.min(DOCK_MAX, Math.max(DOCK_MIN, size)) : DEFAULT.size
	};
}

export const saveDock = (layout: DockLayout) => writeJson(KEY, layout);
