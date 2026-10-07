import {
	getSettings,
	onSettingsChanged,
	setSettings,
	type Settings,
	type SettingsView
} from '#lib/api.js';
import { DEFAULT_APPEARANCE } from '#lib/appearance.js';
import { m } from '#lib/paraglide/messages.js';

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

/**
 * What every settings tab shares: the saved settings, the draft behind the
 * Save button, and the message shown above the tabs.
 */
export class SettingsState {
	view = $state<SettingsView | null>(null);
	draft = $state<Settings>({
		root: '',
		captureHotkey: '',
		hideImmediately: true,
		...DEFAULT_APPEARANCE,
		language: 'system',
		onboarded: false,
		lastSeenVersion: null,
		threadOrder: 'oldest'
	});
	message = $state<{ text: string; error: boolean } | null>(null);
	/** The tab shown: one of the app's, or `plugin:<id>` for a plugin's own. */
	tab = $state('general');

	// Everything else applies as soon as it is picked, so only these wait on
	// the Save button.
	dirty = $derived(
		this.view !== null &&
			(this.draft.root !== this.view.root ||
				this.draft.captureHotkey !== this.view.captureHotkey ||
				this.draft.hideImmediately !== this.view.hideImmediately)
	);

	/** Loads the settings and follows changes made elsewhere. Returns the teardown. */
	start(): () => void {
		const off = [
			onSettingsChanged((settings) => {
				if (!this.view) return;
				this.view = { ...this.view, ...settings };
				this.#sync(this.view);
			})
		];
		void (async () => {
			try {
				this.view = await getSettings();
				this.draft = editable(this.view);
			} catch (e) {
				this.say(String(e), true);
			}
		})();
		return () => off.forEach((p) => void p.then((stop) => stop()));
	}

	say(text: string, error = false) {
		this.message = { text, error };
	}

	/** Saves these at once, past the Save button, whose own edits stay pending. */
	async apply(changes: Partial<Settings>): Promise<boolean> {
		if (!this.view) return false;
		try {
			this.view = await setSettings({ ...editable(this.view), ...changes });
			this.#sync(this.view);
			return true;
		} catch (e) {
			this.say(String(e), true);
			return false;
		}
	}

	/** The Save button. */
	async save() {
		try {
			this.view = await setSettings($state.snapshot(this.draft));
			this.say(m.settings_saved());
		} catch (e) {
			this.say(String(e), true);
		}
	}

	/** The draft takes what is saved, but keeps the edits waiting on the Save button. */
	#sync(saved: Settings) {
		const { root, captureHotkey, hideImmediately } = this.draft;
		this.draft = { ...editable(saved), root, captureHotkey, hideImmediately };
	}
}
