import type { Language } from '#lib/app/i18n.svelte.js';
import { event, invoke } from './invoke.js';

export interface Settings {
	root: string;
	captureHotkey: string;
	/** Hide the capture window on save rather than showing "Saved" first. */
	hideImmediately: boolean;
	/** Accent preset name, see ACCENTS in appearance.ts. */
	accentColor: string;
	/** Font preset name, see FONTS in appearance.ts. */
	fontFamily: string;
	/** Base text size in pixels. */
	fontSize: number;
	/** Corner radius in rem. */
	radius: number;
	/** 'system' follows the OS setting. */
	theme: 'dark' | 'light' | 'system';
	/** A locale such as 'fr', or 'system' to follow the OS language. */
	language: Language;
	/** The first-run walkthrough has been finished or skipped. */
	onboarded: boolean;
	/** The app version whose release notes were last shown; null from 0.1.0. */
	lastSeenVersion: string | null;
	/** Which of a thread's notes it lists first. */
	threadOrder: ThreadOrder;
}

export type ThreadOrder = 'oldest' | 'newest';

export interface SettingsView extends Settings {
	/** The root this run is using; differs from `root` until the next launch. */
	activeRoot: string;
	/** Where the notes go when no folder is chosen: the app's own storage on Android. */
	defaultRoot: string;
}

/** The fields `setSettings` takes, out of a view that carries more. */
export const editable = (s: Settings): Settings => ({
	root: s.root,
	captureHotkey: s.captureHotkey,
	hideImmediately: s.hideImmediately,
	accentColor: s.accentColor,
	fontFamily: s.fontFamily,
	fontSize: s.fontSize,
	radius: s.radius,
	theme: s.theme,
	language: s.language,
	onboarded: s.onboarded,
	lastSeenVersion: s.lastSeenVersion,
	threadOrder: s.threadOrder
});

export const getSettings = () => invoke<SettingsView>('get_settings');

export const setSettings = (settings: Settings) =>
	invoke<SettingsView>('set_settings', { settings });

/** Saves these over the settings as saved now, the others left as they are. */
export const patchSettings = async (changes: Partial<Settings>) =>
	setSettings({ ...editable(await getSettings()), ...changes });

/** Relaunch the app, which is how a new notes root takes effect. */
export const restartApp = () => invoke<void>('restart_app');

/** Android's folder chooser, which the dialog plugin lacks there; null when backed out of. */
export const pickNotesFolder = () => invoke<string | null>('pick_notes_folder');

export const onSettingsChanged = event<Settings>('settings-changed');

/** Fired by the tray's Settings entry. */
export const onOpenSettings = event('open-settings');
