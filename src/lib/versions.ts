/** The numbers of a version, `0.9.1-beta.2` read as 0, 9 and 1. */
const numbers = (version: string) =>
	version
		.split(/[-+]/)[0]
		.split('.')
		.map((n) => Number.parseInt(n, 10) || 0);

/**
 * Negative, zero or positive as `a` is older than, the same as or newer than
 * `b`, number by number as the backend compares them: a missing one counts
 * as 0, and a pre-release or build after `-` or `+` is left out.
 */
export function compareVersions(a: string, b: string): number {
	const [x, y] = [numbers(a), numbers(b)];
	for (let i = 0; i < Math.max(x.length, y.length); i++) {
		if ((x[i] ?? 0) !== (y[i] ?? 0)) return (x[i] ?? 0) - (y[i] ?? 0);
	}
	return 0;
}

/** Whether version `a` is older than `b`. */
export const older = (a: string, b: string) => compareVersions(a, b) < 0;
