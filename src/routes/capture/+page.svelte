<script lang="ts">
	import { onMount } from 'svelte';
	import { hideCapture, onCaptureShown, saveNote } from '$lib/api';

	let draft = $state('');
	let saving = $state(false);
	let error = $state<string | null>(null);
	let input: HTMLTextAreaElement;

	onMount(() => {
		input?.focus();
		// The window is hidden, never closed, so this component stays mounted
		// and the draft survives an Esc.
		const unlisten = onCaptureShown(() => input?.focus());
		return () => void unlisten.then((off) => off());
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

	<div class="flex items-center justify-between text-[11px] text-neutral-500">
		{#if error}
			<span class="text-red-400">{error}</span>
		{:else}
			<span>Ctrl+Enter to save, Esc to dismiss</span>
		{/if}
		<span>{saving ? 'Saving…' : ''}</span>
	</div>
</div>
