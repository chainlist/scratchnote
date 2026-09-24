<script lang="ts">
	import { onMount } from 'svelte';
	import { getSettings, hideCapture, onCaptureShown, onSettingsChanged, saveNote } from '$lib/api';

	let draft = $state('');
	let saving = $state(false);
	let error = $state<string | null>(null);
	let saved = $state(false);
	// SPEC 3.1: hiding at once is the default; the toast is opt-in.
	let hideImmediately = true;
	let input: HTMLTextAreaElement;

	onMount(() => {
		input?.focus();
		// The window is hidden, never closed, so this component stays mounted
		// and the draft survives an Esc.
		const unlisten = [
			onCaptureShown(() => input?.focus()),
			onSettingsChanged((settings) => (hideImmediately = settings.hideImmediately))
		];
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

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save();
		} else if (event.key === 'Escape') {
			event.preventDefault();
			void hideCapture();
		}
	}
</script>

<div
	class="flex h-screen flex-col gap-2 border border-neutral-700 bg-neutral-900 p-3 text-neutral-100"
>
	<textarea
		bind:this={input}
		bind:value={draft}
		onkeydown={onKeydown}
		placeholder="What is on your mind?"
		spellcheck="false"
		class="min-h-0 flex-1 resize-none rounded bg-neutral-800 p-2 text-sm leading-relaxed
			outline-none placeholder:text-neutral-500"></textarea>

	<div class="flex items-center justify-between text-[0.6875rem] text-neutral-500">
		{#if error}
			<span class="text-red-400">{error}</span>
		{:else}
			<span>Ctrl+Enter to save, Esc to dismiss</span>
		{/if}
		<span>{saved ? 'Saved' : saving ? 'Saving…' : ''}</span>
	</div>
</div>
