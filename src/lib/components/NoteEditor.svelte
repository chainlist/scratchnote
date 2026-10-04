<script lang="ts">
	import type { Note } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import MarkdownEditor from '#lib/components/MarkdownEditor.svelte';
	import Trash2Icon from '@lucide/svelte/icons/trash-2';
	import { m } from '#lib/paraglide/messages.js';

	let {
		note = $bindable(),
		onsave,
		ondelete
	}: {
		/** The note being edited; null closes the editor. */
		note: Note | null;
		/** Resolves to an error to show, or null once saved. */
		onsave: (note: Note, body: string) => Promise<string | null>;
		/** Ask to delete; the page confirms. */
		ondelete: (note: Note) => void;
	} = $props();

	let body = $state('');
	let saving = $state(false);
	let error = $state<string | null>(null);

	// Loaded afresh each time a note is opened, so a cancelled edit leaves
	// nothing behind for the next one.
	let loaded: string | null = null;
	$effect(() => {
		const id = note?.id ?? null;
		if (id === loaded) return;
		loaded = id;
		if (!note) return;
		body = note.body;
		error = null;
	});

	async function save() {
		if (!note || saving) return;
		if (body.trim() === '') {
			error = m.editor_empty();
			return;
		}
		saving = true;
		error = await onsave(note, body);
		saving = false;
		if (error === null) note = null;
	}

	function onKeydown(event: KeyboardEvent) {
		if (event.key === 'Enter' && (event.metaKey || event.ctrlKey)) {
			event.preventDefault();
			void save();
		}
	}
</script>

<Dialog.Root
	open={note !== null}
	onOpenChange={(open) => {
		if (!open) note = null;
	}}
>
	<Dialog.Content class="gap-5 sm:max-w-[min(32rem,calc(100%-2rem))]" onkeydown={onKeydown}>
		<Dialog.Header>
			<Dialog.Title>{m.editor_title()}</Dialog.Title>
			<Dialog.Description>
				{#if note}{m.editor_when({ date: note.date, time: note.time })}{/if}
			</Dialog.Description>
		</Dialog.Header>

		<div class="flex flex-col gap-2">
			<span class="text-sm font-medium">{m.editor_text()}</span>
			<MarkdownEditor
				bind:value={body}
				label={m.editor_text()}
				onerror={(message) => (error = message)}
				class="max-h-[calc(12lh+0.75rem)] min-h-[calc(4lh+0.75rem)] w-full rounded-lg border border-input bg-transparent px-2.5 py-1.5 text-sm leading-6 transition-colors focus-within:border-ring dark:bg-input/30"
			/>
		</div>

		{#if error}
			<p class="text-sm text-destructive">{error}</p>
		{/if}

		<Dialog.Footer class="flex-row items-center sm:justify-between">
			<Button variant="ghost" class="text-destructive" onclick={() => note && ondelete(note)}>
				<Trash2Icon />{m.common_delete()}
			</Button>
			<div class="flex gap-2">
				<Button variant="outline" onclick={() => (note = null)}>{m.common_cancel()}</Button>
				<Button onclick={save} disabled={saving}
					>{saving ? m.common_saving() : m.common_save()}</Button
				>
			</div>
		</Dialog.Footer>
	</Dialog.Content>
</Dialog.Root>
