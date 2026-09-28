import changelog from '../../CHANGELOG.md?raw';

/** The one release that did not record the version last opened. */
export const FIRST_RELEASE = '0.1.0';

/** A release in CHANGELOG.md, as release-please writes it. */
export interface Release {
	version: string;
	/** Such as 2026-10-01. */
	date: string;
	/** Such as Features or Bug Fixes, in the changelog's order. */
	sections: { title: string; entries: string[] }[];
}

/**
 * Negative, zero or positive as `a` is older than, the same as or newer than
 * `b`, for plain `x.y.z` versions.
 */
export function compareVersions(a: string, b: string): number {
	const [x, y] = [a, b].map((v) => v.split('.').map(Number));
	for (let i = 0; i < 3; i++) if (x[i] !== y[i]) return x[i] - y[i];
	return 0;
}

/** An entry made for reading in the app: no commit links, and it starts with a capital. */
function tidy(entry: string): string {
	return entry
		.replace(/ \(\[[0-9a-f]{7,}\]\([^)]*\)\)/g, '')
		.replace(/, closes .*$/, '')
		.replace(/^((?:\*\*[^*]+\*\* )?)([a-z])/, (_, scope, first) => scope + first.toUpperCase());
}

/** Every release in CHANGELOG.md, newest first. A Windows checkout has CRLF endings. */
export const releases: Release[] = [];
for (const line of changelog.replace(/\r\n/g, '\n').split('\n')) {
	// `## [0.2.0](https://…/compare/v0.1.0...v0.2.0) (2026-10-01)`, or no link for the first.
	const release = /^## \[?(\d+\.\d+\.\d+)\]?(?:\([^)]*\))? \((\d{4}-\d{2}-\d{2})\)/.exec(line);
	if (release) {
		releases.push({ version: release[1], date: release[2], sections: [] });
		continue;
	}
	const current = releases.at(-1);
	if (!current) continue;
	if (line.startsWith('### ')) current.sections.push({ title: line.slice(4).trim(), entries: [] });
	else if (line.startsWith('* ')) current.sections.at(-1)?.entries.push(tidy(line.slice(2)));
}

/** The releases after `since`, up to `current`, newest first. */
export const releasesSince = (since: string, current: string) =>
	releases.filter(
		(r) => compareVersions(r.version, since) > 0 && compareVersions(r.version, current) <= 0
	);
