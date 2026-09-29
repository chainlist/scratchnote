<script lang="ts">
	import { onMount } from 'svelte';
	import { dev } from '$app/environment';
	import { getVersion } from '@tauri-apps/api/app';
	import { asset } from '$app/paths';
	import { appUpdate } from '$lib/app-update.svelte';
	import { releases, type Release } from '$lib/changelog';
	import { Button } from '$lib/components/ui/button';
	import WhatsNew from '$lib/components/WhatsNew.svelte';
	import DownloadIcon from '@lucide/svelte/icons/download';
	import RefreshCwIcon from '@lucide/svelte/icons/refresh-cw';
	import { m } from '$lib/paraglide/messages';
	import { getLocale } from '$lib/paraglide/runtime';
	import { duration, startupTimes } from '$lib/startup';
	import type { SettingsState } from './state.svelte';
	import StartupDetails from './StartupDetails.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	let version = $state('');
	let releaseNotes = $state<Release[] | null>(null);
	let checking = $state(false);
	/** Said under the version once a check finds nothing newer. */
	let upToDate = $state(false);
	/** The notes of the version running, not the whole changelog. */
	const current = $derived(releases.filter((r) => r.version === version));
	/** Recorded once the first view was up, well before the settings open. */
	const startup = startupTimes();
	let startupShown = $state(false);

	onMount(async () => {
		try {
			version = await getVersion();
		} catch (e) {
			settings.say(String(e), true);
		}
	});

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
		<img src={asset('/icon.svg')} alt="" class="size-14 shrink-0" />
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
						onclick={() => (startupShown = true)}
					>
						{m.settings_startup_details()}
					</Button>
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
					title={appUpdate.failed
						? m.update_failed({ reason: appUpdate.failed })
						: m.update_hint({
								version: appUpdate.update.version,
								current: appUpdate.update.currentVersion
							})}
				>
					<DownloadIcon class={appUpdate.installing ? 'animate-pulse' : ''} />
					{appUpdate.installing
						? m.update_installing()
						: appUpdate.failed
							? m.update_retry()
							: m.update_to({ version: appUpdate.update.version })}
				</Button>
			{:else if !dev}
				<!-- A dev build is not a release, so no release replaces it. -->
				<Button variant="secondary" size="sm" onclick={look} disabled={checking}>
					<RefreshCwIcon class={checking ? 'animate-spin' : ''} />
					{checking ? m.settings_checking() : m.settings_check_updates()}
				</Button>
			{/if}
			{#if current.length}
				<Button variant="secondary" size="sm" onclick={() => (releaseNotes = current)}>
					{m.settings_about_news()}
				</Button>
			{/if}
		</div>
	</div>
</div>

<WhatsNew bind:releases={releaseNotes} />
{#if startup}<StartupDetails bind:open={startupShown} times={startup} />{/if}
