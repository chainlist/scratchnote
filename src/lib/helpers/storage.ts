/**
 * What the window keeps between launches that is not a setting, in its
 * local storage. Storage can be missing or full: a read then finds nothing,
 * and a write or a removal is dropped, so the next launch starts as a first.
 */

/** What `writeJson` kept under `key`; null when nothing was, or it cannot be read. */
export function readJson(key: string): unknown {
	try {
		return JSON.parse(localStorage.getItem(key) ?? 'null');
	} catch {
		return null;
	}
}

export function writeJson(key: string, value: unknown) {
	try {
		localStorage.setItem(key, JSON.stringify(value));
	} catch {
		// Dropped: see above.
	}
}

export function remove(key: string) {
	try {
		localStorage.removeItem(key);
	} catch {
		// Dropped: see above.
	}
}
