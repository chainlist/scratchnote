<script lang="ts">
	import { onMount } from 'svelte';
	import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
	import { goto } from '$app/navigation';
	import { resolve } from '$app/paths';
	import { Button } from '#lib/components/ui/button/index.js';
	import { Switch } from '#lib/components/ui/switch/index.js';
	import { clearResumeStep } from '#lib/app/onboarding.js';
	import { m } from '#lib/paraglide/messages.js';
	import { android } from '#lib/helpers/platform.js';
	import About from './About.svelte';
	import FolderPicker from './FolderPicker.svelte';
	import HotkeyInput from './HotkeyInput.svelte';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	// Tabs are only drawn once the settings have loaded.
	const view = $derived(settings.view!);
	const restartNeeded = $derived(view.root !== view.activeRoot);

	let launchAtLogin = $state(false);

	onMount(async () => {
		// Android has no capture window, hotkey or login to launch at.
		if (android) return;
		try {
			launchAtLogin = await isEnabled();
		} catch (e) {
			settings.say(String(e), true);
		}
	});

	async function toggleLaunchAtLogin() {
		try {
			if (launchAtLogin) await disable();
			else await enable();
			launchAtLogin = await isEnabled();
		} catch (e) {
			settings.say(String(e), true);
		}
	}

	/** From the first step, whatever an earlier run left behind. */
	function runOnboarding() {
		clearResumeStep();
		void goto(resolve('onboarding/'));
	}
</script>

<div class="flex flex-col gap-6">
	<About {settings} />
	<div class={group}>
		<div class="flex flex-col gap-2 px-4 py-3">
			<div class="flex flex-col gap-0.5">
				<span class="text-sm font-medium">{m.settings_root()}</span>
				<p class={hint}>{m.settings_root_hint()}</p>
			</div>
			<FolderPicker
				value={settings.draft.root}
				appStorage={view.defaultRoot}
				onchange={(root) => (settings.draft.root = root)}
				onerror={(message) => settings.say(message, true)}
			/>
			{#if restartNeeded}
				<p class="text-xs text-warning">
					{m.settings_restart_needed({ path: view.activeRoot })}
				</p>
			{/if}
		</div>
		{#if !android}
			<SettingRow id="hotkey" label={m.settings_hotkey()} hint={m.settings_hotkey_hint()}>
				<HotkeyInput
					id="hotkey"
					value={settings.draft.captureHotkey}
					onchange={(hotkey) => (settings.draft.captureHotkey = hotkey)}
				/>
			</SettingRow>
			<SettingRow
				id="hide"
				label={m.settings_hide()}
				hint={settings.draft.hideImmediately ? m.settings_hide_on() : m.settings_hide_off()}
			>
				<Switch id="hide" bind:checked={settings.draft.hideImmediately} />
			</SettingRow>
			<SettingRow id="login" label={m.settings_login()} hint={m.settings_login_hint()}>
				<Switch id="login" checked={launchAtLogin} onCheckedChange={toggleLaunchAtLogin} />
			</SettingRow>
		{/if}
		<SettingRow label={m.settings_onboarding()} hint={m.settings_onboarding_hint()}>
			<Button variant="secondary" size="sm" onclick={runOnboarding}>
				{m.settings_onboarding_run()}
			</Button>
		</SettingRow>
	</div>
</div>
