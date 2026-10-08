<script lang="ts">
	import { open } from '@tauri-apps/plugin-dialog';
	import { Button } from '#lib/components/ui/button/index.js';
	import FolderIcon from '@lucide/svelte/icons/folder';
	import FolderOpenIcon from '@lucide/svelte/icons/folder-open';
	import { m } from '#lib/paraglide/messages.js';

	let {
		value,
		onchange,
		onerror
	}: {
		/** A full path. */
		value: string;
		onchange: (path: string) => void;
		onerror: (message: string) => void;
	} = $props();

	/** The system's folder picker, opened where the current folder is. */
	async function choose() {
		try {
			const picked = await open({
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

<div class="flex items-center gap-2">
	<div
		class="flex h-8 min-w-0 flex-1 items-center gap-2 rounded-lg border bg-muted/40 px-2.5 text-sm"
		title={value}
	>
		<FolderIcon class="size-4 shrink-0 text-muted-foreground" />
		<span class="truncate font-mono text-xs">{value}</span>
	</div>
	<Button variant="secondary" size="sm" onclick={choose}>
		<FolderOpenIcon />
		{m.settings_root_choose()}
	</Button>
</div>
