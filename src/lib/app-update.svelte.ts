import { check, type Update } from '@tauri-apps/plugin-updater';
import { relaunch } from '@tauri-apps/plugin-process';

/**
 * A newer release of the app (SPEC 3.6), shared by the top bar, which looks
 * at launch and once a day, and Settings > About, which looks when asked, so
 * what either finds shows in both.
 */
class AppUpdate {
	update = $state<Update | null>(null);
	installing = $state(false);
	/** Why the last install failed, until the next try. */
	failed = $state<string | null>(null);

	/** Asks the releases for a newer one. Throws when it cannot tell. */
	async look() {
		if (this.installing) return;
		this.update = await check();
	}

	async install() {
		if (!this.update) return;
		this.installing = true;
		this.failed = null;
		try {
			// Windows quits the app to run the installer, which starts it again.
			await this.update.downloadAndInstall();
			await relaunch();
		} catch (e) {
			this.failed = String(e);
			this.installing = false;
		}
	}
}

export const appUpdate = new AppUpdate();
