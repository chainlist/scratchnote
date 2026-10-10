import '@fontsource-variable/atkinson-hyperlegible-next';
import '@fontsource-variable/caveat';
import '@fontsource-variable/geist';
import '@fontsource-variable/ibm-plex-sans';
import '@fontsource-variable/inter';
import '@fontsource-variable/literata';
import '@fontsource-variable/playpen-sans';
import '@fontsource-variable/shantell-sans';
import type { Settings } from '#lib/api.js';
import { m } from '#lib/paraglide/messages.js';
import { writeJson } from '#lib/storage.js';

type Appearance = Pick<Settings, 'accentColor' | 'fontFamily' | 'fontSize' | 'radius' | 'theme'>;

/** Android's system font size, which MainActivity.kt hands over in place of
 *  scaling the text alone; 1 everywhere else. */
const systemScale: number =
	(window as { AndroidText?: { fontScale(): number } }).AndroidText?.fontScale() ?? 1;

/** The root font size the text size setting comes to, in pixels: the rem. */
export function remPixels(fontSize: number) {
	return fontSize * systemScale;
}

const SANS = 'ui-sans-serif, system-ui, sans-serif';

/**
 * Font presets, bundled with the app so they render offline. The first is the
 * default. Only the sans stack changes: dates and counts stay monospace,
 * bar a timeline's times, which take the font with figures of one width.
 */
export const FONTS = [
	{ name: 'inter', label: () => 'Inter', family: `'Inter Variable', ${SANS}` },
	{ name: 'geist', label: () => 'Geist', family: `'Geist Variable', ${SANS}` },
	{ name: 'plex', label: () => 'IBM Plex Sans', family: `'IBM Plex Sans Variable', ${SANS}` },
	{
		name: 'atkinson',
		label: () => 'Atkinson Hyperlegible',
		family: `'Atkinson Hyperlegible Next Variable', ${SANS}`
	},
	{ name: 'literata', label: () => 'Literata', family: `'Literata Variable', ui-serif, serif` },
	{ name: 'caveat', label: () => 'Caveat', family: `'Caveat Variable', ${SANS}` },
	{ name: 'shantell', label: () => 'Shantell Sans', family: `'Shantell Sans Variable', ${SANS}` },
	{ name: 'playpen', label: () => 'Playpen Sans', family: `'Playpen Sans Variable', ${SANS}` },
	{ name: 'system', label: m.settings_font_system, family: SANS }
];

const INK = 'oklch(0.205 0 0)';
const WHITE = 'oklch(0.985 0 0)';

/**
 * Accent presets, a swatch and the text on it for each theme. The light
 * theme's are deeper, so the accent reads as text and its fill holds white
 * text on a near-white page; the dark theme's take dark text. Each pair is at
 * least 4.5:1 both ways (swatch on the page, text on the swatch). Neutral is
 * the palette layout.css already defines; its values here only draw the swatch.
 */
export const ACCENTS = [
	{
		name: 'neutral',
		label: m.settings_accent_neutral,
		dark: { swatch: 'oklch(0.922 0 0)', foreground: INK },
		light: { swatch: INK, foreground: WHITE }
	},
	{
		name: 'blue',
		label: m.settings_accent_blue,
		dark: { swatch: 'oklch(0.623 0.214 259.815)', foreground: INK },
		light: { swatch: 'oklch(0.55 0.214 259.815)', foreground: WHITE }
	},
	{
		name: 'violet',
		label: m.settings_accent_violet,
		dark: { swatch: 'oklch(0.65 0.206 292.717)', foreground: INK },
		light: { swatch: 'oklch(0.57 0.25 292.717)', foreground: WHITE }
	},
	{
		name: 'green',
		label: m.settings_accent_green,
		dark: { swatch: 'oklch(0.723 0.219 149.579)', foreground: INK },
		light: { swatch: 'oklch(0.52 0.143 149.579)', foreground: WHITE }
	},
	{
		name: 'orange',
		label: m.settings_accent_orange,
		dark: { swatch: 'oklch(0.705 0.213 47.604)', foreground: INK },
		light: { swatch: 'oklch(0.55 0.149 47.604)', foreground: WHITE }
	},
	{
		name: 'rose',
		label: m.settings_accent_rose,
		dark: { swatch: 'oklch(0.645 0.246 16.439)', foreground: INK },
		light: { swatch: 'oklch(0.57 0.228 16.439)', foreground: WHITE }
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

/** The light theme's own lightness where the mirror falls short: mirrored,
 *  neutral-900 sits at 0.97 on a 0.985 page, a hover fill no one sees. */
const LIGHT_LIGHTNESS: Partial<Record<number, number>> = { 900: 0.955 };

export const THEMES: { name: Settings['theme']; label: () => string }[] = [
	{ name: 'dark', label: m.settings_theme_dark },
	{ name: 'light', label: m.settings_theme_light },
	{ name: 'system', label: m.settings_theme_system }
];

export const FONT_SIZES = [
	{ px: 14, label: m.settings_size_small },
	{ px: 16, label: m.settings_size_default },
	{ px: 18, label: m.settings_size_large },
	{ px: 20, label: m.settings_size_larger }
];

export const RADII = [
	{ rem: 0, label: m.settings_radius_none },
	{ rem: 0.375, label: m.settings_radius_small },
	{ rem: 0.625, label: m.settings_radius_default },
	{ rem: 0.875, label: m.settings_radius_large }
];

export const DEFAULT_APPEARANCE: Appearance = {
	accentColor: 'neutral',
	fontFamily: 'inter',
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
	const { accentColor, fontFamily, fontSize, radius, theme } = appearance;
	const root = document.documentElement;
	const style = root.style;
	const dark = theme === 'dark' || (theme === 'system' && prefersDark.matches);
	root.classList.toggle('dark', dark);
	// An unknown name, say from a hand-edited settings.json, falls back to neutral.
	const accent = ACCENTS.find((a) => a.name === accentColor && a.name !== 'neutral');
	const colours = accent?.[dark ? 'dark' : 'light'];
	const vars: [string, string | undefined][] = [
		['--primary', colours?.swatch],
		['--ring', colours?.swatch],
		['--primary-foreground', colours?.foreground],
		// Toward the text colour, away from the page: the label on it gains contrast.
		['--primary-hover', colours && `color-mix(in oklch, ${colours.swatch}, var(--foreground) 20%)`]
	];
	// Tint the neutral scale with the accent hue. Surfaces take the most
	// chroma; the steps used for text stay close to grey. Light mode mirrors
	// the scale, so neutral-950 is the page in both themes and the
	// components' hardcoded neutral-* classes flip without a light variant;
	// the one step the mirror leaves too faint takes its own value.
	const hue = colours?.swatch.match(/([\d.]+)\)$/)?.[1];
	NEUTRAL_SCALE.forEach(([step, , chroma], i) => {
		const lightness = dark
			? NEUTRAL_SCALE[i][1]
			: (LIGHT_LIGHTNESS[step] ?? NEUTRAL_SCALE[NEUTRAL_SCALE.length - 1 - i][1]);
		const value = `oklch(${lightness} ${hue ? chroma : 0} ${hue ?? 0})`;
		vars.push([`--color-neutral-${step}`, dark && !hue ? undefined : value]);
	});
	for (const [name, value] of vars) {
		if (value) style.setProperty(name, value);
		else style.removeProperty(name);
	}
	// Tailwind's font-sans reads this variable. An unknown name gets the default.
	const font = FONTS.find((f) => f.name === fontFamily) ?? FONTS[0];
	style.setProperty('--font-sans', font.family);
	style.fontSize = `${remPixels(fontSize)}px`;
	style.setProperty('--radius', `${radius}rem`);
	// app.html restores this at the next launch, before the settings arrive.
	writeJson('appearance', { dark, style: style.cssText });
}

const prefersDark = window.matchMedia('(prefers-color-scheme: dark)');
let last: Appearance | null = null;
// The system theme can change while the app is open.
prefersDark.addEventListener('change', () => {
	if (last?.theme === 'system') applyAppearance(last);
});
