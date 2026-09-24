<script lang="ts">
	import type { Pathname } from '$app/types';
	import { resolve } from '$app/paths';
	import { page } from '$app/state';
	import { locales, localizeHref } from '$lib/paraglide/runtime';
	import './layout.css';
	import { onMount } from 'svelte';
	import favicon from '$lib/assets/favicon.svg';
	import { getSettings, onSettingsChanged } from '$lib/api';
	import { applyAppearance } from '$lib/appearance';

	let { children } = $props();

	// Both windows mount this layout, so an appearance change reaches the
	// capture window too.
	onMount(() => {
		void getSettings().then(applyAppearance);

		const off = onSettingsChanged(applyAppearance);

		return () => void off.then((stop) => stop());
	});
</script>

<svelte:head><link rel="icon" href={favicon} /></svelte:head>
{@render children()}

<div style="display:none">
	{#each locales as locale (locale)}
		<a href={resolve(localizeHref(page.url.pathname, { locale }) as Pathname)}>{locale}</a>
	{/each}
</div>
