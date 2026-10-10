<script lang="ts">
	import type { Note } from '#lib/api.js';
	import { Button } from '#lib/components/ui/button/index.js';
	import * as Dialog from '#lib/components/ui/dialog/index.js';
	import { Input } from '#lib/components/ui/input/index.js';
	import { Label } from '#lib/components/ui/label/index.js';
	import NotePreview from '#lib/components/note/NotePreview.svelte';
	import { m } from '#lib/paraglide/messages.js';

	let {
		note = $bindable(),
		onconfirm
	}: {
		/** The note to turn into a page; null closes the dialog. */
		note: Note | null;
		/** Resolves to an error to show, or null once the page is made. */
		onconfirm: (note: Note, title: string) => Promise<string | null>;
	} = $props();

	// Each note opens the dialog with the title empty and no error.
	const noteId = $derived(note?.id ?? null);
	let title = $derived.by(() => {
		void noteId;
		return '';
	});
	let error = $derived.by((): string | null => {
		void noteId;
		return null;
	});
	let input = $state<HTMLInputElement | null>(null);
	let working = $state(false);

	async function confirm(event?: SubmitEvent) {
		event?.preventDefault();
		if (!note || working) return;
		if (!title.trim()) {
			error = m.pages_needs_title();
			return;
		}
		working = true;
		error = await onconfirm(note, title);
		working = false;
		if (error === null) note = null;
	}
</script>

<Dialog.Root
	open={note !== null}
	onOpenChange={(open) => {
		if (!open) note = null;
	}}
>
	<!-- The title field takes the focus. -->
	<Dialog.Content
		class="sm:max-w-[min(28rem,calc(100%-2rem))]"
		onOpenAutoFocus={(event) => {
			event.preventDefault();
			input?.select();
		}}
	>
		<form class="flex flex-col gap-4" onsubmit={confirm}>
			<Dialog.Header>
				<Dialog.Title>{m.pages_turn_into_title()}</Dialog.Title>
				<Dialog.Description>{m.pages_turn_into_description()}</Dialog.Description>
			</Dialog.Header>
			{#if note}<NotePreview {note} clamp="max-h-[3lh] overflow-hidden" />{/if}
			<div class="flex flex-col gap-2">
				<Label for="page-title">{m.pages_title_label()}</Label>
				<Input
					id="page-title"
					bind:ref={input}
					bind:value={title}
					placeholder={m.pages_title_placeholder()}
				/>
			</div>
			{#if error}
				<p class="text-sm text-destructive">{error}</p>
			{/if}
			<Dialog.Footer>
				<Button variant="outline" type="button" onclick={() => (note = null)}
					>{m.common_cancel()}</Button
				>
				<Button type="submit" disabled={working}>{m.pages_turn_into_confirm()}</Button>
			</Dialog.Footer>
		</form>
	</Dialog.Content>
</Dialog.Root>
