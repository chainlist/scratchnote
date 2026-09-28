<script lang="ts">
	import { onMount, untrack } from 'svelte';
	import { getVersion } from '@tauri-apps/api/app';
	import {
		embeddingModelInfo,
		getSettings,
		modelStatus,
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
	import ModelStatusBar from '$lib/components/ModelStatusBar.svelte';
	import NoteEditor from '$lib/components/NoteEditor.svelte';
	import NoteToPageDialog from '$lib/components/NoteToPageDialog.svelte';
	import PageView from '$lib/components/PageView.svelte';
	import Settings from '$lib/components/settings/Settings.svelte';
	import ViewHeader from '$lib/components/ViewHeader.svelte';
	import WhatsNew from '$lib/components/WhatsNew.svelte';
	import { Button } from '$lib/components/ui/button';
	import * as Dialog from '$lib/components/ui/dialog';
	import PanelRightCloseIcon from '@lucide/svelte/icons/panel-right-close';
	import { m } from '$lib/paraglide/messages';
	import { compareVersions, FIRST_RELEASE, releasesSince, type Release } from '$lib/changelog';
	import { setShell, Shell } from '$lib/shell.svelte';

	let { data, children } = $props();

	// Until a day is shown, the views go back to today.
	const shell = setShell(new Shell(untrack(() => data.today)));

	/** Release notes waiting to be read after an update. */
	let releaseNotes = $state<Release[] | null>(null);

	const mac = navigator.userAgent.includes('Mac');

	function onWindowKeydown(event: KeyboardEvent) {
		if (event.key === '/' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			shell.paletteOpen = !shell.paletteOpen;
		} else if (!mac && event.altKey && (event.key === 'ArrowLeft' || event.key === 'ArrowRight')) {
			// Alt+arrows take the webview through its history, which holds the
			// views. In text they would leave a note or a page half written.
			// On macOS they move by word instead, and the text keeps them.
			const target = event.target as HTMLElement;
			if (target.isContentEditable || target.closest('input, textarea')) event.preventDefault();
		}
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
		const off: Promise<() => void>[] = [];
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
			shell.model = await modelStatus();
			off.push(onModelStatus((status) => (shell.model = status)));
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
		onsearch={() => (shell.paletteOpen = true)}
		onsettings={() => (shell.settingsOpen = true)}
		titleShown={shell.titleCollapsed}
	>
		{#snippet title()}
			{#if shell.title}<ViewHeader compact {...shell.title} />{/if}
		{/snippet}
	</AppHeader>

	<div class="flex min-h-0 flex-1">
		<main bind:offsetWidth={shell.width} class="min-w-0 flex-1 overflow-y-auto px-6 pb-16">
			{@render children()}
		</main>
		{#if shell.docked}
			{@const docked = shell.docked}
			<!-- The page docked beside the view, written in while the notes stay
			     in reach. -->
			<aside
				aria-label={docked.subject ?? m.pages_untitled()}
				class="flex w-(--page-dock) shrink-0 flex-col border-l border-neutral-800"
			>
				<div class="flex justify-end px-3 pt-3">
					<Button
						variant="ghost"
						size="icon-sm"
						onclick={() => (shell.docked = null)}
						aria-label={m.common_close()}
						title={m.common_close()}
						class="text-muted-foreground hover:text-foreground"
					>
						<PanelRightCloseIcon />
					</Button>
				</div>
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
						/>
					{/key}
				</div>
			</aside>
		{/if}
	</div>
</div>

<div class="fixed bottom-3 left-3 z-30">
	<ModelStatusBar status={shell.model} />
</div>

<ChatPanel
	bind:open={shell.chatOpen}
	space={data.spaces.active}
	canChat={shell.canChat}
	modelOff={shell.model.state === 'disabled'}
	docked={shell.docked !== null}
	onopen={(entry) => void shell.openCited(entry)}
/>

<CommandCenter
	bind:open={shell.paletteOpen}
	bind:query={shell.query}
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

<WhatsNew bind:releases={releaseNotes} />

<Dialog.Root bind:open={shell.settingsOpen}>
	<Dialog.Content
		class="h-[min(640px,85vh)] grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden p-0 sm:max-w-[min(48rem,calc(100%-2rem))]"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>
