<script lang="ts">
	import { untrack } from 'svelte';
	import type { Attachment } from 'svelte/attachments';
	import type { PluginSettingTab } from '#lib/plugins/api.js';
	import { clearSettings } from '#lib/plugins/setting-model.js';

	/**
	 * A plugin's settings tab, drawn while it shows: `display` fills the
	 * plugin's element afresh each time, and `hide` is called as it goes.
	 */
	let {
		tab,
		plugin,
		onerror
	}: {
		tab: PluginSettingTab;
		plugin: string;
		/** The plugin failed to draw or close its tab. */
		onerror: (message: string) => void;
	} = $props();

	const show =
		(current: PluginSettingTab): Attachment<HTMLElement> =>
		(el) => {
			const fail = (e: unknown) =>
				onerror(`${plugin}: ${e instanceof Error ? e.message : String(e)}`);
			const { containerEl } = current;
			containerEl.className = 'flex flex-col';
			clearSettings(containerEl);
			el.append(containerEl);
			try {
				// Drawn once as it shows, whatever state the plugin reads to draw it.
				untrack(() => current.display());
			} catch (e) {
				fail(e);
			}
			return () => {
				try {
					current.hide();
				} catch (e) {
					fail(e);
				}
				clearSettings(containerEl);
				containerEl.remove();
			};
		};
</script>

<div {@attach show(tab)}></div>
