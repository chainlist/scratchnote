<script lang="ts">
	import { onDestroy, onMount, untrack } from 'svelte';
	import { fade } from 'svelte/transition';
	import { dev } from '$app/env';
	import { afterNavigate } from '$app/navigation';
	import { navigating } from '$app/state';
	import { getVersion } from '@tauri-apps/api/app';
	import {
		embeddingModelInfo,
		getSettings,
		onEmbeddingStatus,
		onIndexRebuilt,
		onNewPage,
		onNoteUpdated,
		onOpenSettings,
		onPinsChanged,
		onRevealNote,
		onSettingsChanged,
		onSpacesChanged,
		onThreadsChanged,
		setSettings,
		setTrayLabels,
		today,
		type SettingsView
	} from '#lib/api.js';
	import AppHeader from '#lib/components/AppHeader.svelte';
	import CommandCenter from '#lib/components/CommandCenter.svelte';
	import EmbedderBar from '#lib/components/EmbedderBar.svelte';
	import DeleteNoteDialog from '#lib/components/DeleteNoteDialog.svelte';
	import Dock from '#lib/components/Dock.svelte';
	import MoveDialog from '#lib/components/MoveDialog.svelte';
	import NoteToPageDialog from '#lib/components/NoteToPageDialog.svelte';
	import OldChatModelDialog from '#lib/components/OldChatModelDialog.svelte';
	import PageView from '#lib/components/PageView.svelte';
	import PluginPanel from '#lib/components/PluginPanel.svelte';
	import Ribbon from '#lib/components/Ribbon.svelte';
	import RibbonDrawer from '#lib/components/RibbonDrawer.svelte';
	import Settings from '#lib/components/settings/Settings.svelte';
	import ThreadDock from '#lib/components/ThreadDock.svelte';
	import ThreadPicker from '#lib/components/ThreadPicker.svelte';
	import ViewHeader from '#lib/components/ViewHeader.svelte';
	import WhatsNew from '#lib/components/WhatsNew.svelte';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import * as Resizable from '#lib/components/ui/resizable/index.js';
	import { Toaster } from '#lib/components/ui/sonner/index.js';
	import { Spinner } from '#lib/components/ui/spinner/index.js';
	import { m } from '#lib/paraglide/messages.js';
	import { remPixels } from '#lib/appearance.js';
	import { compareVersions, FIRST_RELEASE, releasesSince, type Release } from '#lib/changelog.js';
	import { DOCK_MAX, DOCK_MIN } from '#lib/dock.js';
	import { bindWorkspace } from '#lib/plugins/app.js';
	import { matchesHotkey, runCommand } from '#lib/plugins/commands.js';
	import { registry } from '#lib/plugins/registry.svelte.js';
	import { setShell, Shell } from '#lib/shell.svelte.js';
	import { closeOnBack, startBackGuard } from '#lib/back.svelte.js';
	import { inText, onSwipe, phone } from '#lib/swipe.js';

	let { data, children } = $props();

	// Until a day is shown, the views go back to today.
	const shell = setShell(new Shell(untrack(() => data.today)));

	// The plugins reach the views through it from the start, their pages
	// included, which mount before this layout does.
	onDestroy(bindWorkspace(shell));

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

	let settingsDialog = $state<HTMLElement | null>(null);

	/** Release notes waiting to be read after an update. */
	let releaseNotes = $state<Release[] | null>(null);
	/** Until it is known whether they show, any other dialog at launch waits. */
	let checkingNews = $state(true);

	const mac = navigator.userAgent.includes('Mac');
	// Android has no capture window, so no hotkey brings it up.
	const android = navigator.userAgent.includes('Android');

	function onWindowKeydown(event: KeyboardEvent) {
		if (event.key === '/' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			if (shell.paletteOpen) shell.paletteOpen = false;
			else shell.openPalette();
		} else if (matchesHotkey(event, 'Mod-,')) {
			event.preventDefault();
			shell.paletteOpen = false;
			shell.settingsOpen = true;
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

	// On a phone the settings take the whole screen, and pulling them down
	// from their top closes them, as their X does.
	$effect(() => {
		const dialog = settingsDialog;
		if (!dialog || !phone.current) return;
		const slide = (y: number, animate: boolean) => {
			dialog.style.transition = animate ? 'transform 150ms ease-out' : '';
			dialog.style.transform = y ? `translateY(${y}px)` : '';
		};
		return onSwipe(dialog, {
			axis: 'y',
			// Only from the top: a list scrolled down scrolls back up first.
			accept: (_, target) => {
				if (inText(target)) return false;
				for (let el: Element | null = target; el && el !== dialog; el = el.parentElement)
					if (el.scrollTop > 0) return false;
				return true;
			},
			move: (y) => slide(Math.max(0, y), false),
			end: (y, speed) => {
				if (y > 120 || (y > 30 && speed > 0.5)) shell.settingsOpen = false;
				else slide(0, true);
			}
		});
	});

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
		const off: Promise<() => void>[] = [];
		void (async () => {
			const settings = await getSettings();
			void showReleaseNotes(settings)
				.catch((e) => shell.showError(String(e)))
				.finally(() => (checkingNews = false));

			shell.textSize = remPixels(settings.fontSize);
			shell.threadOrder = settings.threadOrder;
			shell.captureHotkey = android ? '' : settings.captureHotkey;
			off.push(
				onSettingsChanged((changed) => {
					shell.textSize = remPixels(changed.fontSize);
					shell.threadOrder = changed.threadOrder;
					shell.captureHotkey = android ? '' : changed.captureHotkey;
				})
			);
			// A note saved from the capture window lands in another webview.
			off.push(onNoteUpdated((id) => void shell.refresh(id)));
			// The watcher fires this when a daily file is edited outside the app.
			off.push(onIndexRebuilt(() => void shell.refresh()));
			off.push(onOpenSettings(() => (shell.settingsOpen = true)));
			off.push(onNewPage((body) => shell.takeCaptureDraft(body)));
			// The capture window's recall, showing the old note a draft is about.
			off.push(onRevealNote((note) => void shell.openCited(note)));
			// The embed task placing notes in threads, or a thread renamed.
			off.push(onThreadsChanged(() => void shell.loadThreads()));
			void shell.loadThreads();
			off.push(onPinsChanged(() => void shell.loadPins()));
			void shell.loadPins();
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
					<main
						bind:this={shell.main}
						bind:offsetWidth={shell.width}
						class="h-full overflow-y-auto px-6 pb-16"
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
						<PageView
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
<ThreadPicker bind:pick={shell.picking} onconfirm={shell.pickThread} />

<WhatsNew bind:releases={releaseNotes} />
<OldChatModelDialog waiting={checkingNews || releaseNotes !== null} onerror={shell.showError} />

<!-- The whole screen on a phone. The focus goes to the dialog itself rather
     than the first tab, which opened by Ctrl+, would show its focus ring; Tab
     goes on to the tabs. -->
<Dialog.Root bind:open={shell.settingsOpen}>
	<Dialog.Content
		bind:ref={settingsDialog}
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			settingsDialog?.focus();
		}}
		class="h-dvh max-w-full grid-rows-[minmax(0,1fr)] gap-0 overflow-hidden rounded-none p-0 ring-0 sm:h-[min(1000px,85vh)] sm:max-w-[min(1100px,calc(100%-2rem))] sm:rounded-xl sm:ring-1"
	>
		<Settings />
	</Dialog.Content>
</Dialog.Root>

<!-- Says when a command keeps the window waiting (#lib/slow-calls.js), under
     the header (h-12) and its window controls. -->
<Toaster position="top-right" offset={{ top: 56, right: 16 }} />
