<script lang="ts" module>
	const isMac = navigator.userAgent.includes('Mac');

	/** Shows an accelerator the way the OS spells it, e.g. Ctrl+Shift+Space. */
	export const prettyHotkey = (accelerator: string) =>
		accelerator.replace('CommandOrControl', isMac ? '⌘' : 'Ctrl').replace('Super', 'Win');
</script>

<script lang="ts">
	import { Input } from '#lib/components/ui/input/index.js';
	import { m } from '#lib/paraglide/messages.js';

	let {
		id,
		value,
		onchange,
		class: className = ''
	}: {
		id?: string;
		/** An accelerator, as the global-shortcut plugin reads it. */
		value: string;
		onchange: (accelerator: string) => void;
		class?: string;
	} = $props();

	let recording = $state(false);

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

	/** Turns a key press into the accelerator syntax the global-shortcut plugin reads. */
	function recordHotkey(event: KeyboardEvent) {
		// Recording starts on a click, or from the keyboard on Enter or Space.
		// Until then, and once cancelled, keys pass through: Tab moves on and
		// Escape closes the dialog.
		if (!recording) {
			if (event.key === 'Enter' || event.key === ' ') {
				event.preventDefault();
				recording = true;
			}
			return;
		}
		// Tab, with Shift or not, still moves on, and is never a shortcut.
		if (event.key === 'Tab') {
			recording = false;
			return;
		}
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
		onchange([...mods, key].join('+'));
		recording = false;
	}
</script>

<svelte:window onkeydowncapture={cancelRecording} />

<Input
	{id}
	readonly
	value={recording ? m.settings_hotkey_recording() : prettyHotkey(value)}
	onclick={() => (recording = true)}
	onblur={() => (recording = false)}
	onkeydown={recordHotkey}
	class="w-56 cursor-pointer text-center font-mono {recording
		? 'border-primary text-muted-foreground'
		: ''} {className}"
/>
