<script lang="ts">
	import { onMount } from 'svelte';
	import {
		captureToPage,
		getSettings,
		hideCapture,
		listSpaces,
		moveAttachments,
		onCaptureShown,
		onSettingsChanged,
		onSpacesChanged,
		revealNote,
		saveNote,
		type Attachment,
		type SpacesView
	} from '#lib/api.js';
	import MarkdownEditor from '#lib/components/MarkdownEditor.svelte';
	import Recall from '#lib/components/Recall.svelte';
	import SpaceSwitcher from '#lib/components/SpaceSwitcher.svelte';
	import { Button } from '#lib/components/ui/button/index.js';
	import SendHorizontalIcon from '@lucide/svelte/icons/send-horizontal';
	import { relink } from '#lib/markdown.js';
	import { withMention } from '#lib/mentions.js';
	import { m } from '#lib/paraglide/messages.js';
	import { formatHotkey } from '#lib/plugins/commands.js';

	let draft = $state('');
	let saving = $state(false);
	/** What failed in plain words, and the error as it came for its tooltip. */
	let error = $state<{ what: string; detail?: string } | null>(null);
	let saved = $state(false);
	let spaces = $state<SpacesView | null>(null);
	/** The space picked for this draft; until then it goes into the open one. */
	let picked = $state<string | null>(null);
	/** Where the note will be saved. */
	const target = $derived(picked ?? spaces?.active ?? '');
	/**
	 * The space the draft's files were attached to, once it has any. More go
	 * there too, and they all follow the draft if it is sent elsewhere.
	 */
	let attachedIn = $state<string | null>(null);
	/** The files this draft attached, by path. */
	let attached: string[] = [];
	// SPEC 3.1: hiding at once is the default; the toast is opt-in.
	let hideImmediately = true;
	let input: MarkdownEditor;

	const mac = navigator.userAgent.includes('Mac');
	/** The shortcut that picks the nth space, counting from 1. */
	const spaceHotkey = (n: number) => formatHotkey(`Mod-${n}`);

	onMount(() => {
		input?.focus();
		// The window is hidden, never closed, so this component stays mounted
		// and the draft survives an Esc.
		const unlisten = [
			onCaptureShown(() => {
				// A new note starts in the open space; one left half written keeps its own.
				if (draft.trim() === '') forget();
				input?.focus();
			}),
			onSettingsChanged((settings) => (hideImmediately = settings.hideImmediately)),
			onSpacesChanged(followSpaces)
		];
		void listSpaces().then(followSpaces);
		void getSettings().then((settings) => (hideImmediately = settings.hideImmediately));
		return () => unlisten.forEach((p) => void p.then((off) => off()));
	});

	/** A space renamed or deleted from the main window is no longer one to save into. */
	function followSpaces(view: SpacesView) {
		spaces = view;
		const names = view.spaces.map((space) => space.name);
		if (picked !== null && !names.includes(picked)) picked = null;
		if (attachedIn !== null && !names.includes(attachedIn)) {
			attachedIn = null;
			attached = [];
		}
	}

	/** The draft is gone, so its space and its files are too. */
	function forget() {
		picked = null;
		attachedIn = null;
		attached = [];
	}

	function onAttach(files: Attachment[], space: string | undefined) {
		attachedIn ??= space ?? null;
		if (space === attachedIn) attached.push(...files.map((file) => file.path));
	}

	/** Move the files the draft attached into the space it goes to, when that is another. */
	async function bringAttachments(into: string) {
		const from = attachedIn;
		if (from === null || from === into || attached.length === 0) return;
		const moved = await moveAttachments(from, into, attached);
		draft = attached.reduce((text, path, i) => relink(text, path, moved[i]), draft);
		attached = moved;
		attachedIn = into;
	}

	async function save() {
		if (saving) return;
		if (draft.trim() === '') {
			await hideCapture();
			return;
		}
		saving = true;
		error = null;
		try {
			const into = target;
			await bringAttachments(into);
			await saveNote(draft, undefined, into);
			draft = '';
			forget();
			if (!hideImmediately) {
				saved = true;
				await new Promise((resolve) => setTimeout(resolve, 1000));
				saved = false;
			}
			await hideCapture();
		} catch (e) {
			error = { what: m.error_save_note(), detail: String(e) };
		} finally {
			saving = false;
		}
	}

	/** Hand the draft to a new page in the main window, in the space picked (SPEC 3.5). */
	async function toPage() {
		if (saving) return;
		saving = true;
		try {
			const into = target;
			await bringAttachments(into);
			await captureToPage(draft, into);
			draft = '';
			forget();
			error = null;
		} catch (e) {
			error = { what: m.error_capture_page(), detail: String(e) };
		} finally {
			saving = false;
		}
	}

	/**
	 * Ctrl or Cmd and 1 to 9 pick a space by its place in the list, whatever
	 * the keyboard layout, so on AZERTY without Shift too.
	 */
	function pickByNumber(event: KeyboardEvent) {
		const digit = /^Digit([1-9])$/.exec(event.code);
		const mod = mac ? event.metaKey : event.ctrlKey;
		if (!digit || !mod || event.shiftKey || event.altKey || event.defaultPrevented) return false;
		const space = spaces && spaces.spaces.length > 1 && spaces.spaces[Number(digit[1]) - 1];
		if (!space) return false;
		picked = space.name;
		return true;
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey) && event.shiftKey) {
			event.preventDefault();
			void toPage();
		} else if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save();
		} else if (event.key === 'Escape' && !event.defaultPrevented) {
			// The editor takes one first that closes the names `@` offers.
			event.preventDefault();
			void hideCapture();
		} else if (pickByNumber(event)) {
			event.preventDefault();
		}
	}
</script>

<!-- The frame and the footer drag the window; the buttons in the footer
     still click, as Tauri skips them. A window sized too short for the
     formatting buttons and a few lines keeps the lines. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="@container-size flex h-screen flex-col gap-2 border border-neutral-700 bg-neutral-900 p-3 text-neutral-100"
	onkeydown={onKeydown}
	data-tauri-drag-region
>
	<MarkdownEditor
		bind:this={input}
		bind:value={draft}
		placeholder={m.capture_placeholder()}
		label={m.capture_placeholder()}
		space={attachedIn ?? (target || undefined)}
		onerror={(message) => (error = { what: message })}
		onattach={onAttach}
		class="min-h-0 flex-1 rounded bg-neutral-800 p-2 text-sm leading-relaxed [--md-image-height:6rem]"
		toolbarClass="[@container(max-height:8rem)]:hidden"
	/>

	<!-- On a line of its own, so a narrow window still says what went wrong. -->
	{#if error}
		<p
			role="alert"
			class="line-clamp-2 text-[0.6875rem] break-words text-destructive"
			title={error.detail}
		>
			{error.what}
		</p>
	{/if}

	<div
		class="flex items-center justify-between gap-2 text-[0.6875rem] text-meta"
		data-tauri-drag-region="deep"
	>
		{#if !error}
			<!-- An old note on the same thing, once one stands out, in place of
			     the hint (SPEC 6.3). Reading it keeps the draft. -->
			<Recall
				text={draft}
				space={target || undefined}
				onopen={(note) => void revealNote(note)}
				onmention={(name) => {
					draft = withMention(draft, name);
					input?.focus();
				}}
				returnFocus={() => input?.focus()}
			>
				<span class="min-w-0 truncate">{m.capture_hint()}</span>
			</Recall>
		{/if}
		<span class="ml-auto flex shrink-0 items-center gap-2">
			<span aria-live="polite">{saved ? m.capture_saved() : saving ? m.capture_saving() : ''}</span>
			{#if spaces && spaces.spaces.length > 1}
				<!-- Where the note goes: the open space unless another is picked,
				     which stands out. Picked, the text takes the focus back. -->
				<SpaceSwitcher
					view={spaces}
					chosen={target}
					onpick={(name) => (picked = name)}
					hotkey={spaceHotkey}
					title={m.capture_space_title({
						first: spaceHotkey(1),
						last: spaceHotkey(Math.min(spaces.spaces.length, 9))
					})}
					align="end"
					returnFocus={() => input?.focus()}
					class="ml-0 max-w-40 gap-1 px-1.5 text-[0.6875rem] font-normal [&_svg]:size-3 {target ===
					spaces.active
						? 'text-neutral-400'
						: 'text-neutral-100'}"
				/>
			{/if}
			<Button
				size="icon-sm"
				onclick={save}
				disabled={saving}
				aria-label={m.common_save()}
				title={m.common_save()}
			>
				<SendHorizontalIcon />
			</Button>
		</span>
	</div>
</div>
