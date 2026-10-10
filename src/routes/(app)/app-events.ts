import {
	embeddingModelInfo,
	onEmbeddingStatus,
	onIndexRebuilt,
	onNewPage,
	onNoteUpdated,
	onOpenSettings,
	onPinsChanged,
	onRevealNote,
	onSettingsChanged,
	onSpacesChanged,
	onThreadsChanged,
	stopAll,
	type Settings
} from '#lib/api.js';
import { remPixels } from '#lib/app/appearance.js';
import { android } from '#lib/helpers/platform.js';
import type { Shell } from '#lib/shell.svelte.js';

/** What the shell takes from the settings, as they are read and as they change. */
function applySettings(shell: Shell, settings: Settings) {
	shell.textSize = remPixels(settings.fontSize);
	shell.threads.threadOrder = settings.threadOrder;
	// Android has no capture window, so no hotkey brings it up.
	shell.captureHotkey = android ? '' : settings.captureHotkey;
}

/**
 * Keep the shell in step with the settings, read as `settings`, and with
 * what the backend and the other windows do, from the threads and the pins
 * it loads now to the space opened elsewhere. Returns the way to stop.
 */
export function followApp(shell: Shell, settings: Settings, activeSpace: () => string) {
	const off: Promise<() => void>[] = [];
	applySettings(shell, settings);
	off.push(onSettingsChanged((changed) => applySettings(shell, changed)));
	// A note saved from the capture window lands in another webview.
	off.push(onNoteUpdated((id) => void shell.refresh(id)));
	// The watcher fires this when a daily file is edited outside the app.
	off.push(onIndexRebuilt(() => void shell.refresh()));
	off.push(onOpenSettings(() => (shell.settingsOpen = true)));
	off.push(onNewPage((body) => shell.takeCaptureDraft(body)));
	// The capture window's recall, showing the old note a draft is about.
	off.push(onRevealNote((note) => void shell.openCited(note)));
	// The embed task placing notes in threads, or a thread renamed.
	off.push(onThreadsChanged(() => void shell.threads.loadThreads()));
	void shell.threads.loadThreads();
	off.push(onPinsChanged(() => void shell.loadPins()));
	void shell.loadPins();
	off.push(
		onSpacesChanged((view) =>
			view.active !== activeSpace() ? void shell.switchSpace() : void shell.refresh()
		)
	);
	void (async () => {
		shell.embeddingInstalled = (await embeddingModelInfo()).installed;
		off.push(
			onEmbeddingStatus((status) => (shell.embeddingInstalled = status.state === 'installed'))
		);
	})();
	return () => stopAll(...off)();
}
