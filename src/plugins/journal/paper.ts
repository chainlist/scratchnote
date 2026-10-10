/** The paper of the book: its chance, the same every time, and its age. */

/**
 * Numbers in [0, 1) that are always the same for `seed`, so a photo keeps
 * its tilt and a page its stains from one drawing to the next.
 */
export function random(seed: string): () => number {
	let h = 2166136261;
	for (let i = 0; i < seed.length; i++) h = Math.imul(h ^ seed.charCodeAt(i), 16777619);
	return () => {
		h = (h + 0x6d2b79f5) | 0;
		let t = Math.imul(h ^ (h >>> 15), 1 | h);
		t = (t + Math.imul(t ^ (t >>> 7), 61 | t)) ^ t;
		return ((t ^ (t >>> 14)) >>> 0) / 4294967296;
	};
}

export const between = (rand: () => number, min: number, max: number) => min + rand() * (max - min);

/** The picture of a page's age: a cup's ring now and then, a faded blot, foxing. */
export function stains(seed: string): string {
	const rand = random(seed);
	const n = (value: number) => value.toFixed(2);
	let marks = '';
	if (rand() < 0.3) {
		const [x, y, r] = [between(rand, 22, 78), between(rand, 18, 82), between(rand, 9, 13)];
		marks +=
			`<g filter="url(#wobble)" fill="none" stroke="#80521e">` +
			`<circle cx="${n(x)}" cy="${n(y)}" r="${n(r)}" fill="#80521e" fill-opacity=".035" stroke-opacity=".2" stroke-width=".55"/>` +
			`<circle cx="${n(x + 0.5)}" cy="${n(y - 0.4)}" r="${n(r - 0.6)}" stroke-opacity=".07" stroke-width="1.5"/></g>`;
	}
	if (rand() < 0.5) {
		const [x, y] = [between(rand, 10, 90), between(rand, 10, 130)];
		const [rx, ry] = [between(rand, 8, 20), between(rand, 6, 14)];
		marks += `<ellipse cx="${n(x)}" cy="${n(y)}" rx="${n(rx)}" ry="${n(ry)}" fill="#9a6a2c" fill-opacity=".05" filter="url(#blot)"/>`;
	}
	const spots = Math.floor(between(rand, 2, 8));
	for (let i = 0; i < spots; i++) {
		const [x, y, r] = [between(rand, 4, 96), between(rand, 4, 136), between(rand, 0.2, 0.9)];
		const opacity = between(rand, 0.1, 0.28);
		marks += `<circle cx="${n(x)}" cy="${n(y)}" r="${n(r)}" fill="#8a5a24" fill-opacity="${n(opacity)}" filter="url(#soft)"/>`;
	}
	const seedAttr = Math.floor(rand() * 1000);
	const svg =
		`<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 140" preserveAspectRatio="xMidYMid slice"><defs>` +
		`<filter id="wobble"><feTurbulence type="fractalNoise" baseFrequency=".09" numOctaves="2" seed="${seedAttr}"/><feDisplacementMap in="SourceGraphic" scale="2.2"/></filter>` +
		`<filter id="blot" x="-50%" y="-50%" width="200%" height="200%"><feTurbulence type="fractalNoise" baseFrequency=".12" numOctaves="2" seed="${seedAttr + 1}"/><feDisplacementMap in="SourceGraphic" scale="6"/><feGaussianBlur stdDeviation="1.2"/></filter>` +
		`<filter id="soft"><feGaussianBlur stdDeviation=".3"/></filter>` +
		`</defs>${marks}</svg>`;
	return `url("data:image/svg+xml,${encodeURIComponent(svg)}")`;
}
