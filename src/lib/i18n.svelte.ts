import {
	defineCustomClientStrategy,
	isLocale,
	locales,
	type Locale
} from '#lib/paraglide/runtime.js';
import { m } from '#lib/paraglide/messages.js';

/** The language the settings pick, or undefined to follow the OS. */
let chosen = $state<Locale | undefined>(undefined);

// Paraglide asks every strategy on every message call, so reading `$state`
// here makes each translated string in a template re-render when the
// language changes, with no reload. Its own setLocale is never used: the
// language comes from settings.json, through applyLanguage.
defineCustomClientStrategy('custom-settings', {
	getLocale: () => chosen,
	setLocale: () => {}
});

/** 'system' follows the OS language, falling back to English. */
export type Language = 'system' | Locale;

/** Each language in its own tongue, as a language picker shows them. */
const LANGUAGE_NAMES: Record<Locale, string> = {
	en: 'English',
	fr: 'Français',
	es: 'Español',
	de: 'Deutsch',
	it: 'Italiano',
	pt: 'Português'
};

export const LANGUAGES: Language[] = ['system', ...locales];

/** Languages are named in their own tongue; only System follows the open one. */
export const languageName = (language: Language) =>
	language === 'system' ? m.settings_language_system() : LANGUAGE_NAMES[language];

/** An unknown value, say from a hand-edited settings.json, follows the OS. */
export function applyLanguage(language: string) {
	chosen = isLocale(language) ? language : undefined;
}

const MARK = '\u0000';

/**
 * A placeholder value a template styles itself, such as a folder name in
 * monospace. Pass it into a message, then split the result with `parts`.
 */
export const slot = (text: string) => `${MARK}${text}${MARK}`;

/**
 * A message rendered with `slot` values, split around them: the parts at odd
 * indexes are the slots. Each translation may put them in its own order.
 */
export const parts = (message: string) => message.split(MARK);
