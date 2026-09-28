<script lang="ts">
	import { onMount } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import { asset } from '$app/paths';
	import { releases, type Release } from '$lib/changelog';
	import { Button } from '$lib/components/ui/button';
	import WhatsNew from '$lib/components/WhatsNew.svelte';
	import { m } from '$lib/paraglide/messages';
	import type { SettingsState } from './state.svelte';
	import { group, hint } from './styles';

	let { settings }: { settings: SettingsState } = $props();

	let version = $state('');
	let releaseNotes = $state<Release[] | null>(null);

	onMount(async () => {
		try {
			version = await getVersion();
		} catch (e) {
			settings.say(String(e), true);
		}
	});
</script>

<div class={group}>
	<div class="flex items-center gap-4 px-4 py-4">
		<img src={asset('/icon.svg')} alt="" class="size-14 shrink-0" />
		<div class="flex flex-1 flex-col gap-0.5">
			<span class="text-base font-semibold">Scratchnote</span>
			{#if version}
				<span class={hint}>{m.settings_about_version({ version })}</span>
			{/if}
		</div>
		<Button variant="secondary" size="sm" onclick={() => (releaseNotes = releases)}>
			{m.settings_about_news()}
		</Button>
	</div>
</div>

<WhatsNew bind:releases={releaseNotes} />
