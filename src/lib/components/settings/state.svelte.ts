import {
	getSettings,
	modelInfo,
	modelStatus,
	onModelStatus,
	onSettingsChanged,
	setSettings,
	type ModelInfo,
	type ModelStatus,
	type Settings,
	type SettingsView
} from '$lib/api';
import { DEFAULT_APPEARANCE } from '$lib/appearance';
import { m } from '$lib/paraglide/messages';

/** The fields `setSettings` takes, out of a view that carries more. */
const editable = (s: Settings): Settings => ({
	root: s.root,
	captureHotkey: s.captureHotkey,
	hideImmediately: s.hideImmediately,
	modelEnabled: s.modelEnabled,
	modelVariant: s.modelVariant,
	modelPath: s.modelPath,
	idleUnloadMinutes: s.idleUnloadMinutes,
	useGpu: s.useGpu,
	accentColor: s.accentColor,
	fontFamily: s.fontFamily,
	fontSize: s.fontSize,
	radius: s.radius,
	theme: s.theme,
	language: s.language,
	onboarded: s.onboarded,
	lastSeenVersion: s.lastSeenVersion
});

/**
 * What every settings tab shares: the saved settings, the draft behind the
 * Save button, the model in use, and the message shown above the tabs.
 */
export class SettingsState {
	view = $state<SettingsView | null>(null);
	draft = $state<Settings>({
		root: '',
		captureHotkey: '',
		hideImmediately: true,
		modelEnabled: true,
		modelVariant: 'default',
		modelPath: null,
		idleUnloadMinutes: 10,
		useGpu: true,
		...DEFAULT_APPEARANCE,
		language: 'system',
		onboarded: false,
		lastSeenVersion: null
	});
	info = $state<ModelInfo | null>(null);
	model = $state<ModelStatus>({ state: 'absent' });
	message = $state<{ text: string; error: boolean } | null>(null);

	// Everything else applies as soon as it is picked, so only these wait on
	// the Save button.
	dirty = $derived(
		this.view !== null &&
			(this.draft.root !== this.view.root ||
				this.draft.captureHotkey !== this.view.captureHotkey ||
				this.draft.hideImmediately !== this.view.hideImmediately ||
				this.draft.idleUnloadMinutes !== this.view.idleUnloadMinutes)
	);

	/** Loads the settings and follows changes made elsewhere. Returns the teardown. */
	start(): () => void {
		const off = [
			onModelStatus((status) => {
				this.model = status;
				if (status.state !== 'downloading') void this.refreshModels();
			}),
			// A finished download makes that model the one in use.
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
				this.model = await modelStatus();
				await this.refreshModels();
			} catch (e) {
				this.say(String(e), true);
			}
		})();
		return () => off.forEach((p) => void p.then((stop) => stop()));
	}

	say(text: string, error = false) {
		this.message = { text, error };
	}

	async refreshModels() {
		this.info = await modelInfo();
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
		const { root, captureHotkey, hideImmediately, idleUnloadMinutes } = this.draft;
		this.draft = { ...editable(saved), root, captureHotkey, hideImmediately, idleUnloadMinutes };
	}
}
