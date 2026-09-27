<script lang="ts">
	import { onMount } from 'svelte';
	import { dev } from '$app/environment';
	import { check, type Update } from '@tauri-apps/plugin-updater';
	import { relaunch } from '@tauri-apps/plugin-process';
	import { Button } from '$lib/components/ui/button';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { m } from '$lib/paraglide/messages';

	/**
	 * Offers a newer release of the app in the top bar (SPEC 3.6). Checks at
	 * launch and once a day while running, and downloads nothing until clicked.
	 * A failed check stays silent: the app works offline.
	 */
	const DAY = 24 * 60 * 60 * 1000;

	let update = $state<Update | null>(null);
	let installing = $state(false);
	/** Why the last install failed, until the next try. */
	let failed = $state<string | null>(null);

	async function look() {
		if (installing) return;
		try {
			update = await check();
		} catch {
			// Offline, or nothing published yet: the next check tries again.
		}
	}

	onMount(() => {
		// A dev build is not a release, so no release replaces it.
		if (dev) return;
		void look();
		const timer = setInterval(look, DAY);
		return () => clearInterval(timer);
	});

	async function install() {
		if (!update) return;
		installing = true;
		failed = null;
		try {
			// Windows quits the app to run the installer, which starts it again.
			await update.downloadAndInstall();
			await relaunch();
		} catch (e) {
			failed = String(e);
			installing = false;
		}
	}
</script>

{#if update}
	<Button
		variant="ghost"
		size="sm"
		onclick={install}
		disabled={installing}
		title={failed
			? m.update_failed({ reason: failed })
			: m.update_hint({ version: update.version, current: update.currentVersion })}
		class="text-primary hover:text-primary"
	>
		<DownloadIcon class={installing ? 'animate-pulse' : ''} />
		{installing
			? m.update_installing()
			: failed
				? m.update_retry()
				: m.update_to({ version: update.version })}
	</Button>
{/if}
