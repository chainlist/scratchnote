<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { fade } from 'svelte/transition';
	import { dev } from '$app/env';
	import { afterNavigate } from '$app/navigation';
	import { navigating } from '$app/state';
	import { getSettings, setSettings, setTrayLabels, today, type SettingsView } from '#lib/api.js';
	import AppHeader from '#lib/components/layout/AppHeader.svelte';
	import CommandCenter from '#lib/components/layout/CommandCenter.svelte';
	import EmbedderBar from '#lib/components/layout/EmbedderBar.svelte';
	import DeleteNoteDialog from '#lib/components/dialogs/DeleteNoteDialog.svelte';
	import Dock from '#lib/components/layout/Dock.svelte';
	import MoveDialog from '#lib/components/dialogs/MoveDialog.svelte';
	import NoteToPageDialog from '#lib/components/dialogs/NoteToPageDialog.svelte';
	import OldChatModelDialog from '#lib/components/dialogs/OldChatModelDialog.svelte';
	import PageEditor from '#lib/components/page/PageEditor.svelte';
	import PluginPanel from '#lib/components/layout/PluginPanel.svelte';
	import Ribbon from '#lib/components/layout/Ribbon.svelte';
	import RibbonDrawer from '#lib/components/layout/RibbonDrawer.svelte';
	import SettingsDialog from '#lib/components/layout/SettingsDialog.svelte';
	import ThreadDock from '#lib/components/thread/ThreadDock.svelte';
	import ThreadPicker from '#lib/components/thread/ThreadPicker.svelte';
	import ViewHeader from '#lib/components/layout/ViewHeader.svelte';
	import WhatsNew from '#lib/components/dialogs/WhatsNew.svelte';
	import * as Resizable from '#lib/components/ui/resizable/index.js';
	import { Toaster } from '#lib/components/ui/sonner/index.js';
	import { Spinner } from '#lib/components/ui/spinner/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { FIRST_RELEASE, releasesSince, type Release } from '#lib/changelog.js';
	import { compareVersions } from '#lib/helpers/versions.js';
	import { DOCK_MAX, DOCK_MIN } from '#lib/dock.js';
	import { app, bindWorkspace } from '#lib/plugins/app.js';
	import { setShell, Shell } from '#lib/shell.svelte.js';
	import { closeOnBack, startBackGuard } from '#lib/back.svelte.js';
	import { phone } from '#lib/helpers/swipe.js';
	import { followApp } from './app-events.js';
	import { windowKeydown } from './window-keys.js';

	let { data, children } = $props();

	// Until a day is shown, the views go back to today.
	const shell = setShell(new Shell(untrack(() => data.today)));

	// The plugins reach the views through it from the start, their pages
	// included, which mount before this layout does.
	onDestroy(bindWorkspace(shell));

	const onWindowKeydown = windowKeydown(shell);

	// The back arrow goes back no further than the view the app opened on.
	afterNavigate(shell.markFirstEntry);

	// Listed again with the notes whenever a space is opened, made, renamed
	// or deleted.
	$effect(() => {
		shell.spaces = data.spaces;
	});

	/** A view still loading, once it has taken long enough to say so: a quick
	 *  one never flashes the overlay. */
	let loading = $state(false);
	$effect(() => {
		if (!navigating.to) {
			loading = false;
			return;
		}
		const timer = setTimeout(() => (loading = true), 150);
		return () => clearTimeout(timer);
	});

	/** The room beside the ribbon, in px, which the dock and the view share. */
	let room = $state(0);
	/** How narrow and how wide the dock goes, as percentages of that room:
	 *  never under 18rem itself nor leaving the view under 24rem, at any text
	 *  size, as far as the window allows; in a window too narrow for both,
	 *  half each. Whole percentages, so the bounds move only now and then. */
	const dockBounds = $derived.by(() => {
		const rems = room / shell.textSize;
		if (!rems) return { min: DOCK_MIN, max: DOCK_MAX };
		const min = Math.min(50, Math.max(DOCK_MIN, Math.ceil((18 / rems) * 100)));
		const max = Math.max(min, Math.min(DOCK_MAX, Math.floor(100 - (24 / rems) * 100)));
		return { min, max };
	});

	/** Release notes waiting to be read after an update. */
	let releaseNotes = $state<Release[] | null>(null);
	/** Until it is known whether they show, any other dialog at launch waits. */
	let checkingNews = $state(true);

	// Android's Back closes what is open before it leaves the view.
	startBackGuard();
	closeOnBack(
		() => shell.dockOpen,
		() => {
			shell.docked = null;
			shell.panel = null;
			shell.dockedThread = null;
		}
	);

	/**
	 * After an update, the notes of every release since the version last
	 * opened. A settings file without one is from 0.1.0, which kept none.
	 */
	async function showReleaseNotes(settings: SettingsView) {
		const current = app.version;
		const since = settings.lastSeenVersion ?? FIRST_RELEASE;
		if (compareVersions(current, since) <= 0) return;
		const news = releasesSince(since, current);
		if (news.length) releaseNotes = news;
		await setSettings({ ...settings, lastSeenVersion: current });
	}

	onMount(() => {
		let stop = () => {};
		void (async () => {
			const settings = await getSettings();
			void showReleaseNotes(settings)
				.catch((e) => shell.showError(String(e)))
				.finally(() => (checkingNews = false));
			stop = followApp(shell, settings, () => data.spaces.active);
		})();
		return () => stop();
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

<div class="flex h-screen flex-col bg-neutral-950 text-neutral-100">
	<AppHeader
		spaces={data.spaces}
		bind:spacesOpen={shell.spacesOpen}
		onmenu={phone.current ? () => (shell.ribbonOpen = true) : undefined}
		onsearch={shell.openPalette}
		onsettings={() => (shell.settingsOpen = true)}
		titleShown={shell.titleCollapsed}
	>
		{#snippet title()}
			{#if shell.title}<ViewHeader compact {...shell.title} />{/if}
		{/snippet}
	</AppHeader>

	<div class="flex min-h-0 flex-1">
		<!-- On a phone the ribbon waits off screen, for the room. -->
		{#if phone.current}<RibbonDrawer />{:else}<Ribbon />{/if}
		<!-- The dock goes before or after the view, on the side it was moved
		     to. The view's pane stays in place, so the view is never mounted
		     again when the dock opens, closes or moves. -->
		<div bind:clientWidth={room} class="flex min-w-0 flex-1">
			<Resizable.PaneGroup direction="horizontal" class="min-w-0 flex-1">
				{#if shell.dockOpen && shell.dockSide === 'left'}
					{@render dock(1)}
					<Resizable.Handle class="z-10 after:w-2" />
				{/if}
				<Resizable.Pane id="view" order={2} class="relative">
					<!-- On a touch screen the scrollbar keeps its room, which a phone's,
					     drawn over the page, never takes: a day sliding in is laid out
					     as wide as it lands, scrolling or not, under an emulator too. -->
					<main
						bind:this={shell.main}
						bind:offsetWidth={shell.width}
						class="h-full overflow-y-auto px-6 pb-16 pointer-coarse:[scrollbar-gutter:stable]"
					>
						{@render children()}
					</main>
					{#if loading}
						<!-- Over the view being left, until the next one is ready. -->
						<div
							class="absolute inset-0 z-20 flex items-center justify-center bg-background/60"
							transition:fade={{ duration: 120 }}
						>
							<Spinner class="size-6 text-muted-foreground" aria-label={m.view_loading()} />
						</div>
					{/if}
				</Resizable.Pane>
				{#if shell.dockOpen && shell.dockSide === 'right'}
					<Resizable.Handle class="z-10 after:w-2" />
					{@render dock(3)}
				{/if}
			</Resizable.PaneGroup>
		</div>
	</div>
	{#if dev}
		<EmbedderBar />
	{/if}
</div>

{#snippet dock(order: number)}
	<Resizable.Pane
		id="dock"
		{order}
		defaultSize={shell.dockSize}
		minSize={dockBounds.min}
		maxSize={dockBounds.max}
		onResize={shell.resizeDock}
	>
		{#if shell.docked}
			{@const docked = shell.docked}
			<!-- The page docked beside the view, written in while the notes stay
			     in reach. -->
			<Dock label={docked.subject ?? m.pages_untitled()} onclose={() => (shell.docked = null)}>
				<div class="min-h-0 flex-1 overflow-y-auto px-8">
					{#key docked.id}
						<PageEditor
							id={docked.id}
							date={docked.date}
							oncreated={() => void shell.refresh()}
							ondelete={(p) => (shell.deleting = p)}
							onsimilar={shell.canSimilar ? shell.showSimilar : undefined}
							onmove={shell.canMove ? shell.askMove : undefined}
							onopennote={(note) => void shell.openCited(note)}
						/>
					{/key}
				</div>
			</Dock>
		{:else if shell.panel}
			{@const type = shell.panel}
			<PluginPanel {type} onclose={() => shell.closePanel(type)} />
		{:else if shell.dockedThread}
			<ThreadDock id={shell.dockedThread} onclose={() => (shell.dockedThread = null)} />
		{/if}
	</Resizable.Pane>
{/snippet}

<CommandCenter
	bind:open={shell.paletteOpen}
	bind:query={shell.query}
	editor={shell.paletteEditor}
	canMeaning={shell.canSimilar}
	onpick={(note) => void shell.openCited(note)}
	onseeall={(q) => void shell.showResults(q)}
	ontoday={async () => void shell.openDay(await today())}
	oncalendar={() => void shell.showCalendar()}
	onnewpage={shell.newPage}
	onpages={() => void shell.showPages()}
	onmentions={() => void shell.showMentions()}
	onthreads={shell.canSimilar ? () => void shell.showThreads() : undefined}
	onspaces={() => (shell.spacesOpen = true)}
	onsettings={() => (shell.settingsOpen = true)}
/>

<DeleteNoteDialog bind:note={shell.deleting} onconfirm={shell.remove} />

<NoteToPageDialog bind:note={shell.turning} onconfirm={shell.turnIntoPage} />

<MoveDialog bind:note={shell.moving} spaces={shell.spaces} onconfirm={shell.moveTo} />
<ThreadPicker bind:pick={shell.threads.picking} onconfirm={shell.threads.pickThread} />

<WhatsNew bind:releases={releaseNotes} />
<OldChatModelDialog waiting={checkingNews || releaseNotes !== null} onerror={shell.showError} />

<SettingsDialog bind:open={shell.settingsOpen} />

<!-- Says when a command keeps the window waiting (#lib/slow-calls.js), under
     the header (h-12) and its window controls. -->
<Toaster position="top-right" offset={{ top: 56, right: 16 }} />
