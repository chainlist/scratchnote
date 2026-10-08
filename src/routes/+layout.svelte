<script lang="ts">
	import './layout.css';
	import { onMount } from 'svelte';
	import favicon from '#lib/assets/favicon.svg';
	import { getCurrentWindow, LogicalSize } from '@tauri-apps/api/window';
	import { getSettings, onSettingsChanged, type Settings } from '#lib/api.js';
	import { applyAppearance } from '#lib/appearance.js';
	import { followSpace } from '#lib/attachments.svelte.js';
	import { applyLanguage } from '#lib/i18n.svelte.js';
	import { getLocale } from '#lib/paraglide/runtime.js';
	import { recordStartup } from '#lib/startup.js';

	let { children } = $props();

	/** The smallest each window goes at the default text size of 16px, as
	 *  tauri.conf.json and the capture window's builder set them. */
	const MIN_SIZE: Record<string, [number, number]> = { main: [480, 360], capture: [420, 150] };

	/** Everything inside sizes in rem, so the floor grows with the text: a
	 *  larger text never leaves a window too small for its own bar. */
	function fitMinSize(fontSize: number) {
		const window = getCurrentWindow();
		const [width, height] = MIN_SIZE[window.label] ?? [0, 0];
		const scale = fontSize / 16;
		if (width) void window.setMinSize(new LogicalSize(width * scale, height * scale));
	}

	function apply(settings: Settings) {
		applyAppearance(settings);
		applyLanguage(settings.language);
		fitMinSize(settings.fontSize);
	}

	// Both windows mount this layout, so an appearance or language change
	// reaches the capture window too, and both know whose attachments they show.
	onMount(() => {
		// After the first view's own mount work too, such as its editor.
		setTimeout(() => void recordStartup());
		void getSettings().then(apply);
		const off = [onSettingsChanged(apply), followSpace()];
		return () => off.forEach((p) => void p.then((stop) => stop()));
	});

	$effect(() => {
		document.documentElement.lang = getLocale();
	});
</script>

<!-- The name a window goes by until a view gives it its own, as the capture
     window and the onboarding keep. -->
<svelte:head>
	<title>Scratchnote</title>
	<link rel="icon" href={favicon} />
</svelte:head>
{@render children()}
