<script lang="ts">
	import { onMount } from 'svelte';
	import {
		captureToPage,
		getSettings,
		hideCapture,
		listSpaces,
		onCaptureShown,
		onSettingsChanged,
		onSpacesChanged,
		saveNote
	} from '$lib/api';
	import MarkdownEditor from '$lib/components/MarkdownEditor.svelte';
	import { Button } from '$lib/components/ui/button';
	import SendHorizontalIcon from '@lucide/svelte/icons/send-horizontal';
	import { m } from '$lib/paraglide/messages';

	let draft = $state('');
	let saving = $state(false);
	let error = $state<string | null>(null);
	let saved = $state(false);
	/** Where the note will be saved; only named when there is a choice. */
	let space = $state<string | null>(null);
	// SPEC 3.1: hiding at once is the default; the toast is opt-in.
	let hideImmediately = true;
	let input: MarkdownEditor;

	onMount(() => {
		input?.focus();
		// The window is hidden, never closed, so this component stays mounted
		// and the draft survives an Esc.
		const unlisten = [
			onCaptureShown(() => input?.focus()),
			onSettingsChanged((settings) => (hideImmediately = settings.hideImmediately)),
			onSpacesChanged((view) => (space = view.spaces.length > 1 ? view.active : null))
		];
		void listSpaces().then((view) => (space = view.spaces.length > 1 ? view.active : null));
		void getSettings().then((settings) => (hideImmediately = settings.hideImmediately));
		return () => unlisten.forEach((p) => void p.then((off) => off()));
	});

	async function save() {
		if (saving) return;
		if (draft.trim() === '') {
			await hideCapture();
			return;
		}
		saving = true;
		error = null;
		try {
			await saveNote(draft);
			draft = '';
			if (!hideImmediately) {
				saved = true;
				await new Promise((resolve) => setTimeout(resolve, 1000));
				saved = false;
			}
			await hideCapture();
		} catch (e) {
			error = String(e);
		} finally {
			saving = false;
		}
	}

	/** Hand the draft to a new page in the main window (SPEC 3.5). */
	async function toPage() {
		if (saving) return;
		try {
			await captureToPage(draft);
			draft = '';
			error = null;
		} catch (e) {
			error = String(e);
		}
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey) && event.shiftKey) {
			event.preventDefault();
			void toPage();
		} else if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			void hideCapture();
		}
	}
</script>

<!-- The frame and the footer drag the window; the buttons in the footer
     still click, as Tauri skips them. -->
<!-- svelte-ignore a11y_no_static_element_interactions -->
<div
	class="flex h-screen flex-col gap-2 border border-neutral-700 bg-neutral-900 p-3 text-neutral-100"
	onkeydown={onKeydown}
	data-tauri-drag-region
>
	<MarkdownEditor
		bind:this={input}
		bind:value={draft}
		placeholder={m.capture_placeholder()}
		label={m.capture_placeholder()}
		onerror={(message) => (error = message)}
		class="min-h-0 flex-1 rounded bg-neutral-800 p-2 text-sm leading-relaxed [--md-image-height:6rem]"
	/>

	<div
		class="flex items-center justify-between text-[0.6875rem] text-neutral-500"
		data-tauri-drag-region="deep"
	>
		{#if error}
			<span class="text-red-400">{error}</span>
		{:else}
			<span>{m.capture_hint()}</span>
		{/if}
		<span class="flex items-center gap-2">
			<span>{saved ? m.capture_saved() : saving ? m.capture_saving() : ''}</span>
			{#if space}<span class="text-neutral-400" title={m.capture_space_title()}>{space}</span>{/if}
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
