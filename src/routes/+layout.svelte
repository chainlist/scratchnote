<script lang="ts">
	import './layout.css';
	import { onMount } from 'svelte';
	import favicon from '$lib/assets/favicon.svg';
	import { getSettings, onSettingsChanged, type Settings } from '$lib/api';
	import { applyAppearance } from '$lib/appearance';
	import { followSpace } from '$lib/attachments.svelte';
	import { applyLanguage } from '$lib/i18n.svelte';
	import { getLocale } from '$lib/paraglide/runtime';

	let { children } = $props();

	function apply(settings: Settings) {
		applyAppearance(settings);
		applyLanguage(settings.language);
	}

	// Both windows mount this layout, so an appearance or language change
	// reaches the capture window too, and both know whose attachments they show.
	onMount(() => {
		void getSettings().then(apply);
		const off = [onSettingsChanged(apply), followSpace()];
		return () => off.forEach((p) => void p.then((stop) => stop()));
	});

	$effect(() => {
		document.documentElement.lang = getLocale();
	});
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>
{@render children()}
