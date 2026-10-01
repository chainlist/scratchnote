<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import {
		embeddingModelInfo,
		getSettings,
		onEmbeddingStatus,
		onIndexRebuilt,
		onModelStatus,
		onNewPage,
		onNoteEnriched,
		onNoteUpdated,
		onOpenSettings,
		onSettingsChanged,
		onSpacesChanged,
		setSettings,
		setTrayLabels,
		today,
		type SettingsView
	} from '$lib/api';
	import AppHeader from '$lib/components/AppHeader.svelte';
	import ChatPanel from '$lib/components/ChatPanel.svelte';
	import CommandCenter from '$lib/components/CommandCenter.svelte';
	import DeleteNoteDialog from '$lib/components/DeleteNoteDialog.svelte';
	import Dock from '$lib/components/Dock.svelte';
	import ModelStatusBar from '$lib/components/ModelStatusBar.svelte';
	import MoveDialog from '$lib/components/MoveDialog.svelte';
	import NoteEditor from '$lib/components/NoteEditor.svelte';
	import NoteToPageDialog from '$lib/components/NoteToPageDialog.svelte';
	import PageView from '$lib/components/PageView.svelte';
	import PluginPanel from '$lib/components/PluginPanel.svelte';
	import Ribbon from '$lib/components/Ribbon.svelte';
	import Settings from '$lib/components/settings/Settings.svelte';
	import ViewHeader from '$lib/components/ViewHeader.svelte';
	import WhatsNew from '$lib/components/WhatsNew.svelte';
	import * as Dialog from '$lib/components/ui/dialog';
	import * as Resizable from '$lib/components/ui/resizable';
	import { m } from '$lib/paraglide/messages';
	import { compareVersions, FIRST_RELEASE, releasesSince, type Release } from '$lib/changelog';
	import { DOCK_MAX, DOCK_MIN } from '$lib/dock';
	import { bindWorkspace } from '$lib/plugins/app';
	import { matchesHotkey, runCommand } from '$lib/plugins/commands';
	import { registry } from '$lib/plugins/registry.svelte';
	import { setShell, Shell } from '$lib/shell.svelte';

	let { data, children } = $props();

	// Until a day is shown, the views go back to today. The model's state
	// follows its events from here on.
	const shell = setShell(
		new Shell(
			untrack(() => data.today),
			untrack(() => data.model)
		)
	);

	// The plugins reach the views through it from the start, their pages
	// included, which mount before this layout does.
	onDestroy(bindWorkspace(shell));

	// Listed again with the notes whenever a space is opened, made, renamed
	// or deleted.
	$effect(() => {
		shell.spaces = data.spaces;
	});

	/** Release notes waiting to be read after an update. */
	let releaseNotes = $state<Release[] | null>(null);

	const mac = navigator.userAgent.includes('Mac');

	function onWindowKeydown(event: KeyboardEvent) {
		if (event.key === '/' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			if (shell.paletteOpen) shell.paletteOpen = false;
			else shell.openPalette();
		} else if (runPluginHotkey(event)) {
			event.preventDefault();
		} else if (!mac && event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
			// Alt+arrows take the webview through its history, which holds the
			// views. In text they would leave a note or a page half written.
			// On macOS they move by word instead, and the text keeps them.
			const target = event.target as HTMLElement;
			if (target.isContentEditable || target.closest('input, textarea')) event.preventDefault();
		}
	}

	/**
	 * A plugin's command with this hotkey, run. Those on the text are the
	 * editor's own keys, and one the editor already took is left to it.
	 */
	function runPluginHotkey(event: KeyboardEvent) {
		if (event.defaultPrevented) return false;
		// A key without a modifier types, in text.
		const target = event.target as HTMLElement;
		const typing = target.isContentEditable || target.closest('input, textarea');
		if (typing && !(event.ctrlKey || event.metaKey || event.altKey)) return false;
		const command = registry.commands.find(
			(entry) => entry.hotkey && entry.callback && matchesHotkey(event, entry.hotkey)
		);
		if (command) runCommand(command);
		return command !== undefined;
	}

	/**
	 * After an update, the notes of every release since the version last
	 * opened. A settings file without one is from 0.1.0, which kept none.
	 */
	async function showReleaseNotes(settings: SettingsView) {
		const current = await getVersion();
		const since = settings.lastSeenVersion ?? FIRST_RELEASE;
		if (compareVersions(current, since) <= 0) return;
		const news = releasesSince(since, current);
		if (news.length) releaseNotes = news;
		await setSettings({ ...settings, lastSeenVersion: current });
	}

	onMount(() => {
		const off: Promise<() => void>[] = [onModelStatus((status) => (shell.model = status))];
		void (async () => {
			const settings = await getSettings();
			void showReleaseNotes(settings).catch((e) => shell.showError(String(e)));

			shell.textSize = settings.fontSize;
			off.push(onSettingsChanged((changed) => (shell.textSize = changed.fontSize)));
			// A note saved from the capture window lands in another webview.
			off.push(onNoteUpdated(() => void shell.refresh()));
			// The watcher fires this when a daily file is edited outside the app.
			off.push(onIndexRebuilt(() => void shell.refresh()));
			// Enrichment finishing rewrites the note, so the card has to reload.
			off.push(onNoteEnriched(() => void shell.refresh()));
			off.push(onOpenSettings(() => (shell.settingsOpen = true)));
			off.push(onNewPage((body) => shell.takeCaptureDraft(body)));
			off.push(
				onSpacesChanged((view) =>
					view.active !== data.spaces.active ? void shell.switchSpace() : void shell.refresh()
				)
			);

			shell.embeddingInstalled = (await embeddingModelInfo()).installed;
			off.push(
				onEmbeddingStatus((status) => (shell.embeddingInstalled = status.state === 'installed'))
			);
		})();
		return () => off.forEach((p) => void p.then((stop) => stop()));
	});

	// The tray menu lives in Rust; the main window keeps it in the open language.
	$effect(() => {
		void setTrayLabels({
			newNote: m.tray_new_note(),
			open: m.tray_open(),
			settings: m.common_settings(),
			quit: m.tray_quit()
		}).catch((e) => shell.showError(String(e)));
	});
</script>

<svelte:window onkeydown={onWindowKeydown} />

<div
	class="flex h-screen flex-col bg-neutral-950 text-neutral-100"
	data-model-off={shell.model.state === 'disabled' || undefined}
>
	<AppHeader
		spaces={data.spaces}
		onsearch={shell.openPalette}
		onsettings={() => (shell.settingsOpen = true)}
		titleShown={shell.titleCollapsed}
	>
		{#snippet title()}
			{#if shell.title}<ViewHeader compact {...shell.title} />{/if}
		{/snippet}
	</AppHeader>

	<div class="flex min-h-0 flex-1">
		<Ribbon />
		<!-- The dock goes before or after the view, on the side it was moved
		     to. The view's pane stays in place, so the view is never mounted
		     again when the dock opens, closes or moves. -->
		<Resizable.PaneGroup direction="horizontal" class="min-w-0 flex-1">
			{#if shell.dockOpen && shell.dockSide === 'left'}
				{@render dock(1)}
				<Resizable.Handle class="z-10 after:w-2" />
			{/if}
			<Resizable.Pane id="view" order={2} class="relative">
				<main bind:offsetWidth={shell.width} class="h-full overflow-y-auto px-6 pb-16">
					{@render children()}
				</main>
				<ChatPanel
					bind:open={shell.chatOpen}
					space={data.spaces.active}
					canChat={shell.canChat}
					modelOff={shell.model.state === 'disabled'}
					onopen={(entry) => void shell.openCited(entry)}
				/>
			</Resizable.Pane>
			{#if shell.dockOpen && shell.dockSide === 'right'}
				<Resizable.Handle class="z-10 after:w-2" />
				{@render dock(3)}
			{/if}
		</Resizable.PaneGroup>
	</div>
</div>

{#snippet dock(order: number)}
	<Resizable.Pane
		id="dock"
		{order}
		defaultSize={shell.dockSize}
		minSize={DOCK_MIN}
		maxSize={DOCK_MAX}
		onResize={shell.resizeDock}
	>
		{#if shell.docked}
			{@const docked = shell.docked}
			<!-- The page docked beside the view, written in while the notes stay
			     in reach. -->
			<Dock label={docked.subject ?? m.pages_untitled()} onclose={() => (shell.docked = null)}>
				<div class="min-h-0 flex-1 overflow-y-auto px-8">
					{#key docked.id}
						<PageView
							id={docked.id}
							date={docked.date}
							oncreated={() => void shell.refresh()}
							ondelete={(p) => (shell.deleting = p)}
							onretry={shell.retry}
							oncategory={shell.openCategory}
							onsimilar={shell.canSimilar ? shell.showSimilar : undefined}
							onmove={shell.canMove ? shell.askMove : undefined}
						/>
					{/key}
				</div>
			</Dock>
		{:else if shell.panel}
			{@const type = shell.panel}
			<PluginPanel {type} onclose={() => shell.closePanel(type)} />
		{/if}
	</Resizable.Pane>
{/snippet}

<div class="fixed bottom-3 left-3 z-30">
	<ModelStatusBar status={shell.model} />
</div>

<CommandCenter
	bind:open={shell.paletteOpen}
	bind:query={shell.query}
	editor={shell.paletteEditor}
	categories={data.categories}
	canChat={shell.canChat}
	canMeaning={shell.canSimilar}
	onpick={(note) => void shell.openCited(note)}
	onseeall={(q) => void shell.showResults(q)}
	ontoday={async () => void shell.openDay(await today())}
	onnewpage={shell.newPage}
	onpages={() => void shell.showPages()}
	onchat={() => (shell.chatOpen = true)}
	onsettings={() => (shell.settingsOpen = true)}
/>

<NoteEditor
	bind:note={shell.editing}
	categories={data.categoryList}
	onsave={shell.saveEdit}
	ondelete={(n) => (shell.deleting = n)}
/>

<DeleteNoteDialog bind:note={shell.deleting} onconfirm={shell.remove} />

<NoteToPageDialog bind:note={shell.turning} onconfirm={shell.turnIntoPage} />

<MoveDialog bind:note={shell.moving} spaces={shell.spaces} onconfirm={shell.moveTo} />

<WhatsNew bind:releases={releaseNotes} />

<Dialog.Root bind:open={shell.settingsOpen}>
	<Dialog.Content
		class="h-[min(1000px,85vh)] grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-[min(1100px,calc(100%-2rem))]"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>
