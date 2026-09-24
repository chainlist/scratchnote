import type { Settings } from '$lib/api';

type Appearance = Pick<Settings, 'accentColor' | 'fontSize' | 'radius' | 'theme'>;

/**
 * Accent presets. Neutral is the palette layout.css
 * already defines; its values here only draw the swatch.
 */
export const ACCENTS = [
	{
		name: 'neutral',
		label: 'Neutral',
		swatch: 'oklch(0.922 0 0)',
		foreground: 'oklch(0.205 0 0)'
	},
	{
		name: 'blue',
		label: 'Blue',
		swatch: 'oklch(0.623 0.214 259.815)',
		foreground: 'oklch(0.985 0 0)'
	},
	{
		name: 'violet',
		label: 'Violet',
		swatch: 'oklch(0.606 0.25 292.717)',
		foreground: 'oklch(0.985 0 0)'
	},
	{
		name: 'green',
		label: 'Green',
		swatch: 'oklch(0.723 0.219 149.579)',
		foreground: 'oklch(0.205 0 0)'
	},
	{
		name: 'orange',
		label: 'Orange',
		swatch: 'oklch(0.705 0.213 47.604)',
		foreground: 'oklch(0.205 0 0)'
	},
	{
		name: 'rose',
		label: 'Rose',
		swatch: 'oklch(0.645 0.246 16.439)',
		foreground: 'oklch(0.985 0 0)'
	}
];

/** Tailwind's neutral lightness per step, with the chroma a tinted accent adds. */
const NEUTRAL_SCALE: [number, number, number][] = [
	[50, 0.985, 0.002],
	[100, 0.97, 0.003],
	[200, 0.922, 0.005],
	[300, 0.87, 0.007],
	[400, 0.708, 0.012],
	[500, 0.556, 0.016],
	[600, 0.439, 0.018],
	[700, 0.371, 0.02],
	[800, 0.269, 0.02],
	[900, 0.205, 0.018],
	[950, 0.145, 0.015]
];

export const THEMES: { name: Settings['theme']; label: string }[] = [
	{ name: 'dark', label: 'Dark' },
	{ name: 'light', label: 'Light' },
	{ name: 'system', label: 'System' }
];

export const FONT_SIZES = [
	{ px: 14, label: 'Small' },
	{ px: 16, label: 'Default' },
	{ px: 18, label: 'Large' },
	{ px: 20, label: 'Larger' }
];

export const RADII = [
	{ rem: 0, label: 'None' },
	{ rem: 0.375, label: 'Small' },
	{ rem: 0.625, label: 'Default' },
	{ rem: 0.875, label: 'Large' }
];

export const DEFAULT_APPEARANCE: Appearance = {
	accentColor: 'neutral',
	fontSize: 16,
	radius: 0.625,
	theme: 'dark'
};

/**
 * Paints the settings onto the root element. Inline custom properties beat
 * the ones layout.css sets, and Tailwind sizes in rem, so the root font size
 * scales the whole window.
 */
export function applyAppearance(appearance: Appearance) {
	last = appearance;
	const { accentColor, fontSize, radius, theme } = appearance;
	const root = document.documentElement;
	const style = root.style;
	const dark = theme === 'dark' || (theme === 'system' && prefersDark.matches);
	root.classList.toggle('dark', dark);
	// An unknown name, say from a hand-edited settings.json, falls back to neutral.
	const accent = ACCENTS.find((a) => a.name === accentColor && a.name !== 'neutral');
	const vars: [string, string | undefined][] = [
		['--primary', accent?.swatch],
		['--ring', accent?.swatch],
		['--sidebar-primary', accent?.swatch],
		['--sidebar-ring', accent?.swatch],
		['--primary-foreground', accent?.foreground],
		['--sidebar-primary-foreground', accent?.foreground]
	];
	// Tint the neutral scale with the accent hue. Surfaces take the most
	// chroma; the steps used for text stay close to grey. Light mode mirrors
	// the scale, so neutral-950 is the page in both themes and the
	// components' hardcoded neutral-* classes flip without a light variant.
	const hue = accent?.swatch.match(/([\d.]+)\)$/)?.[1];
	NEUTRAL_SCALE.forEach(([step, , chroma], i) => {
		const lightness = NEUTRAL_SCALE[dark ? i : NEUTRAL_SCALE.length - 1 - i][1];
		const value = `oklch(${lightness} ${hue ? chroma : 0} ${hue ?? 0})`;
		vars.push([`--color-neutral-${step}`, dark && !hue ? undefined : value]);
	});
	for (const [name, value] of vars) {
		if (value) style.setProperty(name, value);
		else style.removeProperty(name);
	}
	style.fontSize = `${fontSize}px`;
	style.setProperty('--radius', `${radius}rem`);
}

const prefersDark = window.matchMedia('(prefers-color-scheme: dark)');
let last: Appearance | null = null;
// The system theme can change while the app is open.
prefersDark.addEventListener('change', () => {
	if (last?.theme === 'system') applyAppearance(last);
});
