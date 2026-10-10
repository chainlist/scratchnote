<script lang="ts">
	import { onMount } from 'svelte';
	import { dev } from '$app/env';
	import { appUpdate } from '#lib/app/app-update.svelte.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import { android } from '#lib/helpers/platform.js';

	/**
	 * Offers a newer release of the app in the top bar (SPEC 3.6). Checks at
	 * launch and once a day while running, and downloads nothing until clicked.
	 * A failed check stays silent: the app works offline.
	 */
	const DAY = 24 * 60 * 60 * 1000;

	async function look() {
		try {
			await appUpdate.look();
		} catch {
			// Offline, or nothing published yet: the next check tries again.
		}
	}

	onMount(() => {
		// A dev build is not a release, so no release replaces it. Android
		// gets a new APK rather than updating itself.
		if (dev || android) return;
		void look();
		const timer = setInterval(look, DAY);
		return () => clearInterval(timer);
	});
</script>

{#if appUpdate.update}
	<Button
		variant="ghost"
		size="sm"
		onclick={() => appUpdate.install()}
		disabled={appUpdate.installing}
		title={appUpdate.title}
		class="text-primary hover:text-primary"
	>
		<DownloadIcon class={appUpdate.installing ? 'animate-pulse' : ''} />
		<!-- Left to the icon and its hint in a narrow top bar. -->
		<span class="@max-[48rem]:sr-only">{appUpdate.label}</span>
	</Button>
{/if}
