<script lang="ts">
	import { dev } from '$app/env';
	import { asset } from '$app/paths';
	import { appUpdate } from '#lib/app/app-update.svelte.js';
	import { releases, type Release } from '#lib/app/changelog.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import WhatsNew from '#lib/components/dialogs/WhatsNew.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import { m } from '#lib/paraglide/messages.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { android } from '#lib/helpers/platform.js';
	import { app } from '#lib/plugins/app.js';
	import { duration, startupTimes } from '#lib/app/startup.js';
	import type { SettingsState } from './state.svelte';
	import StartupDetails from './StartupDetails.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	/** Read at startup, before the plugins load. */
	const version = app.version;
	let releaseNotes = $state<Release[] | null>(null);
	let checking = $state(false);
	/** Said under the version once a check finds nothing newer. */
	let upToDate = $state(false);
	/** The notes of the version running, not the whole changelog. */
	const current = $derived(releases.filter((r) => r.version === version));
	/** Recorded once the first view was up, well before the settings open. */
	const startup = startupTimes();
	let startupShown = $state(false);

	/** Unlike the top bar's, this check was asked for, so a failure says why. */
	async function look() {
		checking = true;
		upToDate = false;
		try {
			await appUpdate.look();
			upToDate = !appUpdate.update;
		} catch (e) {
			settings.say(m.settings_update_failed({ reason: String(e) }), true);
		} finally {
			checking = false;
		}
	}
</script>

<div class={group}>
	<div class="flex items-center gap-4 px-4 py-4">
		<img src={asset('icon.svg')} alt="" class="size-14 shrink-0" />

		<div class="flex flex-1 flex-col gap-0.5">
			<span class="text-base font-semibold">Scratchnote</span>
			{#if version}
				<span class={hint}>{m.settings_about_version({ version })}</span>
			{/if}
			{#if startup}
				<span class="{hint} flex items-center gap-1.5">
					{m.settings_startup_total({ time: duration(startup.total, getLocale()) })}
					<Button
						variant="link"
						size="xs"
						class="h-auto p-0 text-xs"
						onclick={() => (startupShown = true)}>{m.settings_startup_details()}</Button
					>
				</span>
			{/if}
			{#if upToDate}
				<span class={hint}>{m.settings_about_up_to_date()}</span>
			{/if}
		</div>
		<div class="flex items-center gap-2">
			{#if appUpdate.update}
				<Button
					size="sm"
					onclick={() => appUpdate.install()}
					disabled={appUpdate.installing}
					title={appUpdate.title}
				>
					<DownloadIcon class={appUpdate.installing ? 'animate-pulse' : ''} />
					{appUpdate.label}
				</Button>
			{:else if !dev && !android}
				<!-- A dev build is not a release, so no release replaces it; Android
				     gets a new APK rather than updating itself. -->
				<Button variant="secondary" size="sm" onclick={look} disabled={checking}>
					<RefreshCwIcon class={checking ? 'animate-spin' : ''} />
					{checking ? m.settings_checking() : m.settings_check_updates()}
				</Button>
			{/if}
			{#if current.length}
				<Button variant="secondary" size="sm" onclick={() => (releaseNotes = current)}
					>{m.settings_about_news()}</Button
				>
			{/if}
		</div>
	</div>
</div>

<WhatsNew bind:releases={releaseNotes} />
{#if startup}<StartupDetails bind:open={startupShown} times={startup} />{/if}
