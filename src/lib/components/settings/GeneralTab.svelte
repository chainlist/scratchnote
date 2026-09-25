<script lang="ts">
	import { onMount } from 'svelte';
	import { disable, enable, isEnabled } from '@tauri-apps/plugin-autostart';
	import { Input } from '$lib/components/ui/input';
	import { Label } from '$lib/components/ui/label';
	import { Switch } from '$lib/components/ui/switch';
	import { m } from '$lib/paraglide/messages';
	import SettingRow from './SettingRow.svelte';
	import type { SettingsState } from './state.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	// Tabs are only drawn once the settings have loaded.
	const view = $derived(settings.view!);
	const restartNeeded = $derived(view.root !== view.activeRoot);

	let launchAtLogin = $state(false);
	let recording = $state(false);

	onMount(async () => {
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

	/**
	 * Escape cancels a recording rather than closing the dialog. Caught on the
	 * window's capture phase, it never reaches the dialog's document listener.
	 */
	function cancelRecording(event: KeyboardEvent) {
		if (!recording || event.key !== 'Escape') return;
		event.preventDefault();
		event.stopPropagation();
		recording = false;
	}

	const isMac = navigator.userAgent.includes('Mac');

	/** Turns a key press into the accelerator syntax the global-shortcut plugin reads. */
	function recordHotkey(event: KeyboardEvent) {
		// Once cancelled, keys (Escape to close the dialog) pass through.
		if (!recording) return;
		event.preventDefault();
		if (['Control', 'Shift', 'Alt', 'Meta'].includes(event.key)) return;

		const mods: string[] = [];
		if (isMac ? event.metaKey : event.ctrlKey) mods.push('CommandOrControl');
		if (isMac ? event.ctrlKey : event.metaKey) mods.push(isMac ? 'Control' : 'Super');
		if (event.altKey) mods.push('Alt');
		if (event.shiftKey) mods.push('Shift');
		// A bare key would fire on every keystroke typed anywhere.
		if (mods.length === 0) return;

		const key = event.code.replace(/^Key/, '').replace(/^Digit/, '');
		settings.draft.captureHotkey = [...mods, key].join('+');
		recording = false;
	}

	/** Shows an accelerator the way the OS spells it, e.g. Ctrl+Shift+Space. */
	const prettyHotkey = (accelerator: string) =>
		accelerator.replace('CommandOrControl', isMac ? '⌘' : 'Ctrl').replace('Super', 'Win');
</script>

<svelte:window onkeydowncapture={cancelRecording} />

<div class={group}>
	<div class="flex flex-col gap-2 px-4 py-3">
		<div class="flex flex-col gap-0.5">
			<Label for="root">{m.settings_root()}</Label>
			<p class={hint}>{m.settings_root_hint()}</p>
		</div>
		<Input id="root" bind:value={settings.draft.root} spellcheck="false" class="font-mono" />
		{#if restartNeeded}
			<p class="text-xs text-amber-500">
				{m.settings_restart_needed({ path: view.activeRoot })}
			</p>
		{/if}
	</div>
	<SettingRow id="hotkey" label={m.settings_hotkey()} hint={m.settings_hotkey_hint()}>
		<Input
			id="hotkey"
			readonly
			value={recording ? m.settings_hotkey_recording() : prettyHotkey(settings.draft.captureHotkey)}
			onfocus={() => (recording = true)}
			onblur={() => (recording = false)}
			onkeydown={recordHotkey}
			class="w-56 cursor-pointer text-center font-mono {recording
				? 'border-primary text-muted-foreground'
				: ''}"
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
</div>
