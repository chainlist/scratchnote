<script lang="ts">
	import { onDestroy, onMount, tick } from 'svelte';
	import {
		createPage,
		finishPage,
		getPage,
		onIndexRebuilt,
		onNoteUpdated,
		renamePage,
		stopAll,
		updatePage,
		type Note
	} from '#lib/api.js';
	import MarkdownEditor from '#lib/components/MarkdownEditor.svelte';
	import DayAhead, { aheadLabel } from '#lib/components/DayAhead.svelte';
	import Recall from '#lib/components/Recall.svelte';
	import * as DropdownMenu from '#lib/components/ui/dropdown-menu/index.js';
	import CalendarXIcon from '@lucide/svelte/icons/calendar-x';
	import EllipsisIcon from '@lucide/svelte/icons/ellipsis';
	import FolderInputIcon from '@lucide/svelte/icons/folder-input';
	import RouteIcon from '@lucide/svelte/icons/route';
	import RouteOffIcon from '@lucide/svelte/icons/route-off';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import WaypointsIcon from '@lucide/svelte/icons/waypoints';
	import { wordCount } from '#lib/markdown.js';
	import { joinText, pageDraft } from '#lib/page-draft.js';
	import { withMention } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { getShell } from '#lib/shell.svelte.js';

	let {
		id,
		date,
		oncreated,
		ondelete,
		onsimilar,
		onmove,
		onopennote,
		own = false
	}: {
		/** The page to open, or null for a new one. */
		id: string | null;
		/** The day a new page goes on. */
		date: string;
		/** A new page got its title and was saved. */
		oncreated: (page: Note) => void;
		/** Ask to delete; the page confirms. */
		ondelete: (page: Note) => void;
		/** Left out without the embedding model. */
		onsimilar?: (page: Note) => void;
		/** Ask to move it to another space, once its text is saved. Left out with one space. */
		onmove?: (page: Note) => void;
		/** Show an old note the text is about, where it is. */
		onopennote?: (note: Note) => void;
		/** Open as a view of its own, not in the dock: its title is then the
		 *  view's heading and the window's title. */
		own?: boolean;
	} = $props();

	/** The page as last saved or read, null until a new one has a title. */
	let page = $state<Note | null>(null);
	let title = $state('');
	let body = $state('');
	/** The text the file holds, to tell typing apart from what was loaded. */
	let savedBody = $state('');
	/** The text the view opened with: recall waits for it to change. */
	let openedWith = $state('');
	let loading = $state(true);
	/** What failed in plain words, and the error as it came for its tooltip. */
	let error = $state<{ what: string; detail?: string } | null>(null);
	let saving = $state(false);
	/** Something was saved since the view opened, which the status then says. */
	let saved = $state(false);
	let editor = $state<MarkdownEditor | null>(null);
	let titleInput = $state<HTMLInputElement | null>(null);
	let destroyed = false;

	/** How long typing has to stop before the text saves itself. */
	const QUIET_MS = 1000;
	let timer: ReturnType<typeof setTimeout> | undefined;
	/** Every write waits for the one before, so a new page is created once
	 *  and each save sends the text as it is by then. */
	let chain: Promise<void> = Promise.resolve();
	function queue(write: () => Promise<void>) {
		chain = chain.then(write);
		return chain;
	}

	const shell = getShell();
	const words = $derived(wordCount(body));
	const cleanTitle = (raw: string) => raw.split(/\s+/).filter(Boolean).join(' ');

	onMount(() => {
		void (async () => {
			if (id === null) {
				title = pageDraft.title;
				body = pageDraft.body;
				savedBody = body;
				openedWith = body;
				loading = false;
				await tick();
				titleInput?.focus();
				return;
			}
			try {
				load(await getPage(id));
				openedWith = body;
			} catch (e) {
				error = { what: m.error_load_page(), detail: String(e) };
			}
			loading = false;
			await tick();
			editor?.focus();
		})();

		// An edit in another editor.
		return stopAll(
			onNoteUpdated((changed) => void refresh(changed)),
			onIndexRebuilt(() => void refresh())
		);
	});

	onDestroy(() => {
		destroyed = true;
		clearTimeout(timer);
		// A page still being created is finished by `saveTitle`.
		if (!page) {
			keepDraft();
			return;
		}
		// The last save first, then the embedding model gets the page, once (SPEC 3.5).
		void queue(async () => {
			await saveBody();
			if (page) await finishPage(page.id).catch(() => {});
		});
	});

	function load(fresh: Note) {
		page = fresh;
		// Not while it is being typed in.
		if (document.activeElement !== titleInput) title = fresh.subject ?? '';
		body = fresh.body;
		savedBody = fresh.body;
	}

	/** Take in what changed on disk, unless it would overwrite unsaved typing. */
	async function refresh(changed?: string) {
		const current = page;
		if (!current || (changed !== undefined && changed !== current.id)) return;
		try {
			const fresh = await getPage(current.id);
			if (body !== savedBody || timer !== undefined) {
				page = { ...fresh, subject: current.subject, body: current.body };
			} else {
				load(fresh);
			}
		} catch {
			// Gone, say deleted in another editor; the day view will say so.
		}
	}

	function keepDraft() {
		pageDraft.title = title;
		pageDraft.body = body;
	}

	/** Text handed over from the capture window, after what is typed. */
	export function addText(text: string) {
		body = joinText(body, text);
	}

	// The editor binds `body`, so a change to it is typing, or a reload.
	$effect(() => {
		if (loading || body === savedBody) return;
		clearTimeout(timer);
		timer = setTimeout(() => {
			timer = undefined;
			void queue(saveBody);
		}, QUIET_MS);
	});

	async function saveBody() {
		if (!page) {
			keepDraft();
			return;
		}
		const text = body;
		if (text.trim() === savedBody.trim()) return;
		saving = true;
		try {
			const fresh = await updatePage(page.id, text);
			savedBody = text;
			page = { ...fresh, subject: page.subject };
			saved = true;
			error = null;
		} catch (e) {
			error = { what: m.error_save_page(), detail: String(e) };
		} finally {
			saving = false;
		}
	}

	/** Enter or leaving the field: a new page is created, an existing one retitled. */
	async function saveTitle() {
		const wanted = cleanTitle(title);
		if (!wanted) {
			// A page cannot lose its title; a new one waits for one.
			if (page) title = page.subject ?? '';
			return;
		}
		if (page?.subject === wanted) {
			title = wanted;
			return;
		}
		saving = true;
		try {
			if (page) {
				const renamed = await renamePage(page.id, wanted);
				page = { ...renamed, body: page.body };
				title = renamed.subject ?? wanted;
			} else {
				const text = body;
				const created = await createPage(wanted, text, date);
				page = created;
				savedBody = text;
				title = created.subject ?? wanted;
				pageDraft.title = '';
				pageDraft.body = '';
				// Leaving the view is what blurred the title: stay left, and
				// the page is done with.
				if (!destroyed) oncreated(created);
				else await finishPage(created.id).catch(() => {});
			}
			saved = true;
			error = null;
		} catch (e) {
			error = { what: m.error_save_page(), detail: String(e) };
		} finally {
			saving = false;
		}
	}

	/** The text typed so far is saved first, so none of it stays behind. */
	async function move(ask: (page: Note) => void) {
		clearTimeout(timer);
		timer = undefined;
		await queue(saveBody);
		// A save that failed says why in the status line, and the page stays.
		if (page && body.trim() === savedBody.trim()) ask(page);
	}

	function onTitleKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter') {
			event.preventDefault();
			// Moving on blurs the field, which saves the title.
			editor?.focus();
		} else if (event.key === 'Escape' && page) {
			event.preventDefault();
			title = page.subject ?? '';
			editor?.focus();
		}
	}

	function onKeydown(event: KeyboardEvent) {
		// Saving is automatic; this only makes it now.
		if (event.key === 's' && (event.ctrlKey || event.metaKey)) {
			event.preventDefault();
			clearTimeout(timer);
			timer = undefined;
			void queue(saveBody);
		}
	}

	const status = $derived(
		error
			? error.what
			: saving
				? m.common_saving()
				: !page && body.trim()
					? m.pages_needs_title()
					: saved && body === savedBody
						? m.pages_saved()
						: ''
	);
</script>

<svelte:head>
	{#if own}<title>{title.trim() || m.pages_untitled()} · Scratchnote</title>{/if}
</svelte:head>

<!-- svelte-ignore a11y_no_static_element_interactions -->
<div class="flex flex-col gap-4 pb-16" onkeydown={onKeydown}>
	<!-- The title is a field; the view's heading, for a screen reader's list
	     of them, says the same. -->
	{#if own}<h1 class="sr-only">{title.trim() || m.pages_untitled()}</h1>{/if}
	<div class="flex items-start gap-2">
		<input
			bind:this={titleInput}
			bind:value={title}
			onblur={() => {
				// Switching to another app blurs the field too; a title half typed
				// then must not become a file name. It saves once focus moves on
				// inside the window, or on Enter.
				if (document.hasFocus()) void queue(saveTitle);
			}}
			onkeydown={onTitleKeydown}
			placeholder={m.pages_title_placeholder()}
			aria-label={m.pages_title_label()}
			disabled={loading}
			class="min-w-0 flex-1 border-b border-transparent bg-transparent text-2xl font-semibold tracking-tight text-neutral-100 outline-none placeholder:text-meta focus-visible:border-ring"
		/>
		{#if page}
			{@const current = page}
			<DropdownMenu.Root>
				<DropdownMenu.Trigger
					aria-label={m.note_actions()}
					title={m.note_actions()}
					class="mt-0.5 flex size-7 cursor-pointer items-center justify-center rounded text-neutral-400 hover:bg-neutral-800 hover:text-neutral-200 focus-visible:bg-neutral-800 focus-visible:text-neutral-200 data-[state=open]:bg-neutral-800 data-[state=open]:text-neutral-200"
				>
					<EllipsisIcon class="size-4" />
				</DropdownMenu.Trigger>
				<DropdownMenu.Content align="end" class="w-max max-w-80 min-w-52">
					{#if onsimilar}
						<DropdownMenu.Item onSelect={() => onsimilar(current)}>
							<WaypointsIcon />{m.note_similar()}
						</DropdownMenu.Item>
					{/if}
					{#if onmove}
						<DropdownMenu.Item onSelect={() => void move(onmove)}>
							<FolderInputIcon />{m.move_to()}
						</DropdownMenu.Item>
					{/if}
					{#if current.on}
						{@const on = current.on}
						<DropdownMenu.Item onSelect={() => void shell.clearDayAhead(current)}>
							<CalendarXIcon />{m.day_ahead_clear({ date: aheadLabel(on) })}
						</DropdownMenu.Item>
					{/if}
					{#if shell.canSimilar}
						<DropdownMenu.Item onSelect={() => shell.askThread(current)}>
							<RouteIcon />{shell.threadOf(current.id) ? m.thread_move() : m.thread_add()}
						</DropdownMenu.Item>
					{/if}
					{#if shell.threadOf(current.id)}
						<DropdownMenu.Item onSelect={() => void shell.keepOut(current, true)}>
							<RouteOffIcon />{m.thread_leave()}
						</DropdownMenu.Item>
					{:else if shell.keptOut(current.id)}
						<DropdownMenu.Item onSelect={() => void shell.keepOut(current, false)}>
							<RouteIcon />{m.thread_rejoin()}
						</DropdownMenu.Item>
					{/if}
					<DropdownMenu.Separator />
					<DropdownMenu.Item variant="destructive" onSelect={() => ondelete(current)}>
						<Trash2Icon />{m.common_delete()}
					</DropdownMenu.Item>
				</DropdownMenu.Content>
			</DropdownMenu.Root>
		{/if}
	</div>

	<div class="-mt-2 flex flex-wrap items-center gap-x-3 gap-y-1 text-xs text-meta">
		{#if page}<span class="font-mono">{page.date} {page.time}</span>{/if}
		<span>{m.pages_words({ count: words })}</span>
		{#if page?.on}<DayAhead on={page.on} />{/if}
		<span class={error ? 'text-destructive' : ''} title={error?.detail} aria-live="polite"
			>{status}</span
		>
		{#if !loading}
			<Recall
				text={body}
				initial={openedWith}
				exclude={page?.id}
				onopen={onopennote}
				onmention={(name) => (body = withMention(body, name))}
				class="max-w-full"
			/>
		{/if}
	</div>

	{#if !loading}
		<MarkdownEditor
			bind:this={editor}
			bind:value={body}
			placeholder={m.pages_body_placeholder()}
			label={m.pages_body_label()}
			onerror={(message) => (error = { what: message })}
			class="min-h-[50vh] text-base leading-7 text-neutral-200"
			toolbarClass="sticky top-0 z-10 bg-neutral-950"
		/>
	{/if}
</div>
