<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog';
	import { pickNotesFolder } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import SmartphoneIcon from '@lucide/svelte/icons/smartphone';
	import { m } from '#lib/paraglide/messages.js';
	import { android } from '#lib/platform.js';

	let {
		value,
		appStorage,
		onchange,
		onerror
	}: {
		/** A full path. */
		value: string;
		/** The app's own storage, which Android offers to go back to. */
		appStorage?: string;
		onchange: (path: string) => void;
		onerror: (message: string) => void;
	} = $props();

	/** Its path means nothing to a phone's owner, so it goes by name. */
	const inAppStorage = $derived(android && value === appStorage);

	/** The system's folder picker, opened where the current folder is. */
	async function choose() {
		try {
			// The dialog plugin cannot pick folders on Android, so the app asks its
			// own chooser there, which also asks for access to a shared folder.
			const picked = android
				? await pickNotesFolder()
				: await open({
						directory: true,
						defaultPath: value || undefined,
						title: m.settings_root_dialog()
					});
			if (typeof picked === 'string') onchange(picked);
		} catch (e) {
			onerror(m.error_pick_folder({ reason: String(e) }));
		}
	}
</script>

<div class="flex flex-wrap items-center gap-2">
	<div
		class="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-lg border bg-muted/40 px-2.5 text-sm"
		title={value}
	>
		{#if inAppStorage}
			<SmartphoneIcon class="size-4 shrink-0 text-muted-foreground" />
			<span class="truncate text-xs">{m.settings_root_app_storage()}</span>
		{:else}
			<FolderIcon class="size-4 shrink-0 text-muted-foreground" />
			<span class="truncate font-mono text-xs">{value}</span>
		{/if}
	</div>
	{#if android && appStorage && !inAppStorage}
		<Button variant="secondary" size="sm" onclick={() => onchange(appStorage)}>
			<SmartphoneIcon />
			{m.settings_root_app_storage()}
		</Button>
	{/if}
	<Button variant="secondary" size="sm" onclick={choose}>
		<FolderOpenIcon />
		{m.settings_root_choose()}
	</Button>
</div>
